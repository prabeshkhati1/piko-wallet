mod import;
mod model;
mod store;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use model::*;
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use store::Store;
use tower_http::services::ServeDir;

struct App {
    store: Store,
    origin: String,
    setup_token: Option<String>,
    secure: bool,
    attempts: Mutex<Vec<Instant>>,
    telegram: Option<(String, String)>,
}
type Shared = Arc<App>;
struct Error(StatusCode, String);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}
impl From<String> for Error {
    fn from(e: String) -> Self {
        let status = if e.starts_with("Data changed") || e.starts_with("Bill changed") {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        };
        Self(status, e)
    }
}
type Api<T> = std::result::Result<T, Error>;
fn unauthorized() -> Error {
    Error(StatusCode::UNAUTHORIZED, "Please unlock your wallet".into())
}
fn token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|x| x.trim().strip_prefix("piko_session="))
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn authorized(app: &App, headers: &HeaderMap) -> bool {
    token(headers).is_some_and(|t| app.store.has_session(&digest(t)))
}
fn require(app: &App, headers: &HeaderMap) -> Api<()> {
    if authorized(app, headers) {
        Ok(())
    } else {
        Err(unauthorized())
    }
}
fn revision(h: &HeaderMap) -> Api<u64> {
    h.get("if-match")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim_matches('"').parse().ok())
        .ok_or_else(|| {
            Error(
                StatusCode::PRECONDITION_REQUIRED,
                "Refresh the wallet before saving".into(),
            )
        })
}
async fn guard(State(app): State<Shared>, req: axum::extract::Request, next: Next) -> Response {
    let is_api = req.uri().path().starts_with("/api/");
    if is_api
        && ![Method::GET, Method::HEAD, Method::OPTIONS].contains(req.method())
        && req.headers().get("origin").and_then(|v| v.to_str().ok()) != Some(app.origin.as_str())
    {
        return Error(
            StatusCode::FORBIDDEN,
            "Request origin is not allowed".into(),
        )
        .into_response();
    }
    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("content-security-policy", HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'"));
    h.insert(
        "cache-control",
        HeaderValue::from_static(if is_api { "no-store" } else { "no-cache" }),
    );
    if app.secure {
        h.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    response
}
async fn auth_status(State(app): State<Shared>, h: HeaderMap) -> Api<Json<Value>> {
    Ok(Json(
        json!({"configured": app.store.password_hash()?.is_some(), "authenticated": authorized(&app,&h), "setup_token_required": app.setup_token.is_some()}),
    ))
}
#[derive(Deserialize)]
struct Credentials {
    password: String,
    setup_token: Option<String>,
}
async fn login(State(app): State<Shared>, Json(c): Json<Credentials>) -> Api<Response> {
    authenticate(app, c, false).await
}
async fn setup(State(app): State<Shared>, Json(c): Json<Credentials>) -> Api<Response> {
    authenticate(app, c, true).await
}
async fn authenticate(app: Shared, c: Credentials, setup: bool) -> Api<Response> {
    {
        let mut attempts = app.attempts.lock().map_err(|_| {
            Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Login unavailable".into(),
            )
        })?;
        attempts.retain(|t| t.elapsed() < Duration::from_secs(300));
        if attempts.len() >= 10 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many attempts. Try again in five minutes.".into(),
            ));
        }
        attempts.push(Instant::now());
    }
    if c.password.len() < 12 || c.password.len() > 128 {
        return Err(Error(
            StatusCode::BAD_REQUEST,
            "Use a password of 12 to 128 characters".into(),
        ));
    }
    if setup {
        if app
            .setup_token
            .as_ref()
            .is_some_and(|t| c.setup_token.as_deref() != Some(t))
        {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Setup key is incorrect".into(),
            ));
        }
        let hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(c.password.as_bytes(), &salt)
                .map(|p| p.to_string())
                .map_err(|_| "Password setup failed".to_string())
        })
        .await
        .map_err(|_| {
            Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Password setup failed".into(),
            )
        })??;
        app.store.setup(&hash)?;
    } else {
        let hash = app
            .store
            .password_hash()?
            .ok_or_else(|| Error(StatusCode::BAD_REQUEST, "Set up your wallet first".into()))?;
        let good = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&hash).ok().is_some_and(|p| {
                Argon2::default()
                    .verify_password(c.password.as_bytes(), &p)
                    .is_ok()
            })
        })
        .await
        .unwrap_or(false);
        if !good {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "Password is incorrect".into(),
            ));
        }
    }
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    app.store
        .session(&digest(&token), Utc::now().timestamp() + 30 * 24 * 3600)?;
    let cookie = format!(
        "piko_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000{}",
        if app.secure { "; Secure" } else { "" }
    );
    let mut response = Json(json!({"ok":true})).into_response();
    response
        .headers_mut()
        .insert("set-cookie", cookie.parse().unwrap());
    Ok(response)
}
async fn logout(State(app): State<Shared>, h: HeaderMap) -> Json<Value> {
    if let Some(t) = token(&h) {
        app.store.logout(&digest(t));
    }
    Json(json!({"ok":true}))
}
async fn state(State(app): State<Shared>, h: HeaderMap) -> Api<Json<Value>> {
    require(&app, &h)?;
    let d = app.store.read()?;
    let balances: BTree = d
        .wallets
        .iter()
        .map(|w| (w.id.clone(), d.balance(w)))
        .collect();
    let total: i64 = d
        .wallets
        .iter()
        .filter(|w| w.include_in_balance)
        .map(|w| d.in_base(d.balance(w), &w.currency))
        .sum();
    Ok(Json(
        json!({"data":d,"balances":balances,"total":total,"today":d.today(),"telegram_enabled":app.telegram.is_some()}),
    ))
}
type BTree = std::collections::BTreeMap<String, i64>;
async fn save(
    State(app): State<Shared>,
    h: HeaderMap,
    Path(kind): Path<String>,
    Json(value): Json<Value>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    let rev = revision(&h)?;
    app.store.mutate(Some(rev), |d| {
        macro_rules! upsert {
            ($field:ident,$ty:ty) => {{
                let item: $ty = serde_json::from_value(value.clone())
                    .map_err(|e| format!("Invalid form: {e}"))?;
                if let Some(old) = d.$field.iter_mut().find(|x| x.id == item.id) {
                    *old = item;
                } else {
                    d.$field.push(item);
                }
            }};
        }
        match kind.as_str() {
            "wallets" => {
                let item: Wallet =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                if d.wallets
                    .iter()
                    .any(|w| w.id == item.id && w.currency != item.currency)
                    && (d.transactions.iter().any(|t| {
                        t.wallet_id == item.id || t.to_wallet_id.as_deref() == Some(&item.id)
                    }) || d.bills.iter().any(|b| b.wallet_id == item.id))
                {
                    return Err("A wallet with records cannot change currency".into());
                }
                upsert!(wallets, Wallet);
            }
            "categories" => upsert!(categories, Category),
            "transactions" => {
                let item: Transaction =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                if item.bill_id.is_some()
                    && !d.transactions.iter().any(|t| {
                        t.id == item.id && t.bill_id == item.bill_id && t.bill_due == item.bill_due
                    })
                {
                    return Err("Use mark paid to create a bill payment".into());
                }
                if d.transactions.iter().any(|t| {
                    t.id == item.id && (t.bill_id != item.bill_id || t.bill_due != item.bill_due)
                }) {
                    return Err("Payment references cannot be changed".into());
                }
                upsert!(transactions, Transaction);
            }
            "bills" => {
                upsert!(bills, Bill);
                d.reminders.retain(|r| {
                    r.bill_id.as_ref().is_none_or(|id| {
                        !d.bills.iter().any(|b| {
                            b.id == *id && (b.completed || b.paused || r.due != Some(b.next_due))
                        })
                    })
                });
            }
            "budgets" => upsert!(budgets, Budget),
            "loans" => upsert!(loans, Loan),
            "settings" => {
                let settings: Settings =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                if settings.currency != d.settings.currency
                    && (!d.transactions.is_empty() || !d.budgets.is_empty())
                {
                    return Err("Base currency is locked once transactions or budgets exist".into());
                }
                d.settings = settings;
            }
            _ => return Err("Unknown record type".into()),
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
async fn delete(
    State(app): State<Shared>,
    h: HeaderMap,
    Path((kind, id)): Path<(String, String)>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    app.store.mutate(Some(revision(&h)?), |d| {
        match kind.as_str() {
            "wallets" => { if d.transactions.iter().any(|t| t.wallet_id==id || t.to_wallet_id.as_deref()==Some(&id)) || d.bills.iter().any(|b| b.wallet_id==id) { return Err("Remove this wallet's transactions and bills first".into()); } d.wallets.retain(|x| x.id!=id); },
            "categories" => { if d.transactions.iter().any(|t| t.category_id.as_deref()==Some(&id)) || d.bills.iter().any(|b| b.category_id.as_deref()==Some(&id)) || d.budgets.iter().any(|b| b.category_id.as_deref()==Some(&id)) { return Err("This category is still used by a transaction, bill, or budget".into()); } d.categories.retain(|x| x.id!=id); },
            "transactions" => { if d.transactions.iter().any(|t| t.id==id && t.bill_id.is_some()) { return Err("A settled bill payment cannot be deleted. Edit its amount or note instead.".into()); } d.transactions.retain(|x| x.id!=id); },
            "bills" => { d.bills.retain(|x| x.id!=id); d.reminders.retain(|x| x.bill_id.as_deref()!=Some(&id)); },
            "budgets" => d.budgets.retain(|x| x.id!=id), "loans" => d.loans.retain(|x| x.id!=id),
            _ => return Err("Unknown record type".into()),
        }; Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct Pay {
    due: NaiveDate,
    date: NaiveDate,
}
async fn pay(
    State(app): State<Shared>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Pay>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    let id = app
        .store
        .mutate(None, |d| d.pay_bill(&id, body.due, body.date))?;
    Ok(Json(json!({"ok":true,"transaction_id":id})))
}
#[derive(Deserialize)]
struct Snooze {
    until: NaiveDate,
}
async fn snooze(
    State(app): State<Shared>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Snooze>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    app.store.mutate(Some(revision(&h)?), |d| {
        if body.until <= d.today() {
            return Err("Choose a future snooze date".into());
        }
        let b = d
            .bills
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or("Bill not found")?;
        b.snoozed_until = Some(body.until);
        for r in &mut d.reminders {
            if r.bill_id.as_deref() == Some(&id) {
                r.read = true;
                r.delivered = true;
            }
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
async fn read_reminders(State(app): State<Shared>, h: HeaderMap) -> Api<Json<Value>> {
    require(&app, &h)?;
    app.store.mutate(None, |d| {
        for r in &mut d.reminders {
            r.read = true;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct Repay {
    id: String,
    amount: i64,
    date: NaiveDate,
    note: String,
    wallet_id: Option<String>,
}
async fn repay(
    State(app): State<Shared>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Repay>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    app.store.mutate(Some(revision(&h)?), |d| {
        let loan = d
            .loans
            .iter()
            .find(|l| l.id == id)
            .ok_or("Loan not found")?
            .clone();
        if loan.records.iter().any(|r| r.id == body.id) {
            return Ok(());
        }
        if let Some(wid) = &body.wallet_id {
            let w = d.wallet(wid)?;
            if w.currency != loan.currency {
                return Err("Choose a wallet in the loan's currency".into());
            }
            d.transactions.push(Transaction {
                id: body.id.clone(),
                kind: if loan.kind == "lent" {
                    "income"
                } else {
                    "expense"
                }
                .into(),
                wallet_id: wid.clone(),
                to_wallet_id: None,
                amount: body.amount,
                to_amount: None,
                category_id: None,
                title: format!("{} repayment", loan.name),
                note: body.note.clone(),
                date: body.date,
                tags: vec!["loan".into()],
                bill_id: None,
                bill_due: None,
            });
        }
        d.loans
            .iter_mut()
            .find(|l| l.id == id)
            .unwrap()
            .records
            .push(LoanRecord {
                id: body.id,
                amount: body.amount,
                date: body.date,
                note: body.note,
            });
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
async fn export(State(app): State<Shared>, h: HeaderMap) -> Api<Response> {
    require(&app, &h)?;
    let mut r = Json(app.store.read()?).into_response();
    r.headers_mut().insert(
        "content-disposition",
        HeaderValue::from_static("attachment; filename=piko-wallet-backup.json"),
    );
    Ok(r)
}
async fn import_piko(
    State(app): State<Shared>,
    h: HeaderMap,
    Json(mut backup): Json<Data>,
) -> Api<Json<Value>> {
    require(&app, &h)?;
    backup.validate()?;
    app.store.mutate(Some(revision(&h)?), |d| {
        backup.revision = d.revision;
        *d = backup;
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
async fn import_ivy(State(app): State<Shared>, h: HeaderMap, body: Bytes) -> Api<Json<Value>> {
    require(&app, &h)?;
    let mut backup = import::ivy(&body)?;
    let count = backup.transactions.len();
    app.store.mutate(Some(revision(&h)?), |d| {
        backup.revision = d.revision;
        *d = backup;
        Ok(())
    })?;
    Ok(Json(json!({"ok":true,"transactions":count})))
}
async fn preview_ivy(State(app): State<Shared>, h: HeaderMap, body: Bytes) -> Api<Json<Value>> {
    require(&app, &h)?;
    let d = import::ivy(&body)?;
    Ok(Json(
        json!({"wallets":d.wallets.len(),"transactions":d.transactions.len(),"bills":d.bills.len(),"budgets":d.budgets.len(),"loans":d.loans.len(),"currency":d.settings.currency}),
    ))
}
async fn scheduler(app: Shared) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        if let Ok(mut d) = app.store.read() {
            if d.reminders_at(Utc::now()) {
                let _ = app.store.mutate(None, |state| {
                    state.reminders_at(Utc::now());
                    Ok(())
                });
            }
        }
        if let Some((token, chat)) = &app.telegram {
            if let Ok(d) = app.store.read() {
                for r in d.reminders.iter().filter(|r| !r.delivered) {
                    // Claim before sending: at-most-once avoids duplicate financial notifications after a crash.
                    if app
                        .store
                        .mutate(None, |d| {
                            let target = d
                                .reminders
                                .iter_mut()
                                .find(|x| x.id == r.id)
                                .ok_or("Reminder removed")?;
                            if target.delivered {
                                return Err("Already delivered".into());
                            }
                            target.delivered = true;
                            Ok(())
                        })
                        .is_err()
                    {
                        continue;
                    }
                    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
                    let ok = client
                        .post(url)
                        .json(&json!({"chat_id":chat,"text":format!("Piko Wallet\n{}",r.text)}))
                        .send()
                        .await
                        .ok()
                        .is_some_and(|r| r.status().is_success());
                    if !ok {
                        eprintln!(
                            "Telegram delivery failed; reminder remains available in Piko Wallet."
                        );
                    }
                }
            }
        }
    }
}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let port = env::var("SERVER_PORT")
        .or_else(|_| env::var("PORT"))
        .unwrap_or_else(|_| "33239".into());
    let origin = env::var("PIKO_ORIGIN")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{port}"))
        .trim_end_matches('/')
        .to_string();
    let secure = origin.starts_with("https://");
    let setup_token = env::var("PIKO_SETUP_TOKEN").ok().filter(|s| !s.is_empty());
    if secure && setup_token.is_none() {
        return Err("Set PIKO_SETUP_TOKEN before exposing setup on a public HTTPS domain".into());
    }
    let host = env::var("PIKO_HOST")
        .unwrap_or_else(|_| if secure { "0.0.0.0" } else { "127.0.0.1" }.into());
    let db = PathBuf::from(env::var("PIKO_DATABASE").unwrap_or_else(|_| "data/piko.db".into()));
    let static_path = env::var("PIKO_STATIC_DIR").unwrap_or_else(|_| "static".into());
    if !std::path::Path::new(&static_path)
        .join("index.html")
        .exists()
    {
        return Err("Piko's static/index.html is missing. Run from the release folder or set PIKO_STATIC_DIR.".into());
    }
    let telegram = env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .zip(env::var("TELEGRAM_CHAT_ID").ok());
    let app = Arc::new(App {
        store: Store::open(&db)?,
        origin: origin.clone(),
        setup_token,
        secure,
        attempts: Mutex::new(vec![]),
        telegram,
    });
    let router = Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"ok":true,"app":"Piko Wallet"})) }),
        )
        .route("/api/auth/status", get(auth_status))
        .route("/api/auth/setup", post(setup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/state", get(state))
        .route("/api/save/{kind}", put(save))
        .route("/api/delete/{kind}/{id}", post(delete))
        .route("/api/bills/{id}/pay", post(pay))
        .route("/api/bills/{id}/snooze", post(snooze))
        .route("/api/loans/{id}/repay", post(repay))
        .route("/api/reminders/read", post(read_reminders))
        .route("/api/export", get(export))
        .route("/api/import/piko", post(import_piko))
        .route("/api/import/ivy", post(import_ivy))
        .route("/api/import/ivy/preview", post(preview_ivy))
        .fallback_service(ServeDir::new(static_path).append_index_html_on_directories(true))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app.clone());
    tokio::spawn(scheduler(app));
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    println!(
        "Piko Wallet listening on {origin}. Database: {}",
        db.display()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
