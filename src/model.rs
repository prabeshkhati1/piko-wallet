//! Web adaptation of Ivy Wallet's accounts, transactions and planned payments.
//! Copyright 2026 Piko Wallet contributors. GPL-3.0-only; see NOTICE.md.
use chrono::{Datelike, Days, Months, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_AMOUNT: i64 = 9_000_000_000_000;
pub fn id() -> String {
    Uuid::new_v4().to_string()
}
pub fn digits(code: &str) -> u32 {
    match code {
        "JPY" | "KRW" | "VND" | "CLP" | "ISK" => 0,
        "KWD" | "BHD" | "OMR" | "JOD" | "TND" => 3,
        _ => 2,
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Settings {
    pub name: String,
    pub currency: String,
    pub timezone: String,
    pub reminder_days: Vec<u32>,
    pub reminder_hour: u32,
    pub daily_entry_reminder: bool,
    /// millionths of a base-currency unit per one unit of the foreign currency
    pub rates: BTreeMap<String, i64>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            name: "My wallet".into(),
            currency: "USD".into(),
            timezone: "UTC".into(),
            reminder_days: vec![3, 1, 0],
            reminder_hour: 9,
            daily_entry_reminder: false,
            rates: BTreeMap::from([("USD".into(), 1_000_000)]),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Wallet {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub opening_balance: i64,
    pub color: String,
    pub icon: String,
    pub include_in_balance: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub id: String,
    pub kind: String,
    pub wallet_id: String,
    pub to_wallet_id: Option<String>,
    pub amount: i64,
    pub to_amount: Option<i64>,
    pub category_id: Option<String>,
    pub title: String,
    pub note: String,
    pub date: NaiveDate,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub bill_id: Option<String>,
    #[serde(default)]
    pub bill_due: Option<NaiveDate>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Bill {
    pub id: String,
    pub title: String,
    pub wallet_id: String,
    pub category_id: Option<String>,
    pub amount: i64,
    pub kind: String,
    pub next_due: NaiveDate,
    pub recurrence: String,
    pub interval: u32,
    pub anchor_day: u32,
    pub paused: bool,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub snoozed_until: Option<NaiveDate>,
    #[serde(default)]
    pub note: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Budget {
    pub id: String,
    pub name: String,
    pub amount: i64,
    pub category_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Loan {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub amount: i64,
    pub currency: String,
    pub due: Option<NaiveDate>,
    pub note: String,
    #[serde(default)]
    pub records: Vec<LoanRecord>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LoanRecord {
    pub id: String,
    pub amount: i64,
    pub date: NaiveDate,
    pub note: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Reminder {
    pub id: String,
    pub bill_id: Option<String>,
    pub due: Option<NaiveDate>,
    pub text: String,
    pub created_at: String,
    pub read: bool,
    pub delivered: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Data {
    pub version: u32,
    pub revision: u64,
    pub settings: Settings,
    pub wallets: Vec<Wallet>,
    pub categories: Vec<Category>,
    pub transactions: Vec<Transaction>,
    pub bills: Vec<Bill>,
    pub budgets: Vec<Budget>,
    pub loans: Vec<Loan>,
    pub reminders: Vec<Reminder>,
}
impl Default for Data {
    fn default() -> Self {
        let categories = [
            ("Food & drinks", "#F29F30", "coffee"),
            ("Shopping", "#6B4DFF", "bag"),
            ("Transport", "#4CC3FF", "car"),
            ("Home", "#14CC9E", "home"),
            ("Health", "#FF4060", "heart"),
            ("Entertainment", "#C34CFF", "play"),
            ("Salary", "#14CC9E", "income"),
            ("Other", "#929096", "grid"),
        ]
        .iter()
        .map(|(name, color, icon)| Category {
            id: id(),
            name: (*name).into(),
            color: (*color).into(),
            icon: (*icon).into(),
        })
        .collect();
        Self {
            version: 1,
            revision: 0,
            settings: Settings::default(),
            wallets: vec![],
            categories,
            transactions: vec![],
            bills: vec![],
            budgets: vec![],
            loans: vec![],
            reminders: vec![],
        }
    }
}
fn text(s: &str, max: usize, required: bool) -> Result<()> {
    if s.len() > max || (required && s.trim().is_empty()) {
        Err("Text is empty or too long".into())
    } else {
        Ok(())
    }
}
fn amount(a: i64) -> Result<()> {
    if a <= 0 || a > MAX_AMOUNT {
        Err("Amount must be positive and within the supported range".into())
    } else {
        Ok(())
    }
}
fn currency(s: &str) -> Result<()> {
    if s.len() != 3 || !s.bytes().all(|c| c.is_ascii_uppercase()) {
        Err("Use a three-letter uppercase currency code".into())
    } else {
        Ok(())
    }
}
fn color(s: &str) -> Result<()> {
    if s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("Invalid color".into())
    }
}
fn ids<'a>(values: impl Iterator<Item = &'a String>) -> Result<()> {
    let mut seen = HashSet::new();
    for value in values {
        text(value, 120, true)?;
        if !seen.insert(value) {
            return Err("Duplicate record ID".into());
        }
    }
    Ok(())
}
impl Data {
    pub fn wallet(&self, id: &str) -> Result<&Wallet> {
        self.wallets
            .iter()
            .find(|w| w.id == id)
            .ok_or_else(|| "Wallet not found".into())
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err("Unsupported backup version".into());
        }
        if self.transactions.len() > 50_000
            || self.wallets.len() > 100
            || self.categories.len() > 500
            || self.bills.len() > 1000
            || self.loans.len() > 1000
            || self.budgets.len() > 500
            || self.reminders.len() > 1000
        {
            return Err("Personal-use record limit exceeded".into());
        }
        text(&self.settings.name, 100, true)?;
        currency(&self.settings.currency)?;
        self.settings
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| "Invalid timezone")?;
        if self.settings.reminder_hour > 23
            || self.settings.reminder_days.len() > 10
            || self.settings.reminder_days.iter().any(|x| *x > 365)
        {
            return Err("Invalid reminder settings".into());
        }
        if self.settings.rates.get(&self.settings.currency) != Some(&1_000_000) {
            return Err("Base currency rate must equal 1".into());
        }
        for (c, rate) in &self.settings.rates {
            currency(c)?;
            if *rate <= 0 || *rate > 1_000_000_000_000 {
                return Err("Invalid exchange rate".into());
            }
        }
        ids(self.wallets.iter().map(|x| &x.id))?;
        ids(self.categories.iter().map(|x| &x.id))?;
        ids(self.transactions.iter().map(|x| &x.id))?;
        ids(self.bills.iter().map(|x| &x.id))?;
        ids(self.budgets.iter().map(|x| &x.id))?;
        ids(self.loans.iter().map(|x| &x.id))?;
        ids(self.reminders.iter().map(|x| &x.id))?;
        let has_category = |id: &Option<String>| -> Result<()> {
            if id
                .as_ref()
                .is_some_and(|v| !self.categories.iter().any(|c| c.id == *v))
            {
                Err("Category not found".into())
            } else {
                Ok(())
            }
        };
        for w in &self.wallets {
            text(&w.name, 100, true)?;
            text(&w.icon, 30, true)?;
            color(&w.color)?;
            currency(&w.currency)?;
            if w.opening_balance.unsigned_abs() > MAX_AMOUNT as u64 {
                return Err("Opening balance is too large".into());
            }
            if !self.settings.rates.contains_key(&w.currency) {
                return Err(format!("Set an exchange rate for {} first", w.currency));
            }
        }
        let mut paid_bills = HashSet::new();
        for t in &self.transactions {
            let w = self.wallet(&t.wallet_id)?;
            amount(t.amount)?;
            has_category(&t.category_id)?;
            text(&t.title, 160, true)?;
            text(&t.note, 2000, false)?;
            if t.date.year() < 1900 || t.date.year() > 2200 {
                return Err("Date must be between 1900 and 2200".into());
            }
            if t.tags.len() > 20 {
                return Err("Too many tags".into());
            }
            for tag in &t.tags {
                text(tag, 40, true)?;
            }
            match t.kind.as_str() {
                "expense" | "income" => {
                    if t.to_wallet_id.is_some() || t.to_amount.is_some() {
                        return Err("Only transfers can have a destination".into());
                    }
                }
                "transfer" => {
                    let to = self.wallet(
                        t.to_wallet_id
                            .as_deref()
                            .ok_or("Choose a destination wallet")?,
                    )?;
                    if w.id == to.id {
                        return Err("Transfer needs different wallets".into());
                    }
                    amount(t.to_amount.ok_or("Enter the received amount")?)?;
                    if w.currency == to.currency && t.to_amount != Some(t.amount) {
                        return Err("Same-currency transfer amounts must match".into());
                    }
                }
                _ => return Err("Invalid transaction type".into()),
            }
            if t.bill_id.is_some() != t.bill_due.is_some() {
                return Err("Incomplete payment reference".into());
            }
            if let (Some(bill), Some(due)) = (&t.bill_id, t.bill_due) {
                if !paid_bills.insert((bill, due)) {
                    return Err("Bill already paid for this date".into());
                }
            }
        }
        for b in &self.bills {
            self.wallet(&b.wallet_id)?;
            has_category(&b.category_id)?;
            amount(b.amount)?;
            text(&b.title, 160, true)?;
            text(&b.note, 2000, false)?;
            if !["expense", "income"].contains(&b.kind.as_str()) {
                return Err("Bills must be income or expense".into());
            }
            if !["none", "day", "week", "month", "year"].contains(&b.recurrence.as_str())
                || b.interval == 0
                || b.interval > 365
                || b.anchor_day == 0
                || b.anchor_day > 31
            {
                return Err("Invalid recurrence".into());
            }
            if b.next_due.year() < 1900 || b.next_due.year() > 2200 {
                return Err("Bill date must be between 1900 and 2200".into());
            }
        }
        for b in &self.budgets {
            amount(b.amount)?;
            text(&b.name, 100, true)?;
            has_category(&b.category_id)?;
        }
        for l in &self.loans {
            amount(l.amount)?;
            currency(&l.currency)?;
            text(&l.name, 100, true)?;
            text(&l.note, 2000, false)?;
            if !["borrowed", "lent"].contains(&l.kind.as_str()) {
                return Err("Invalid loan type".into());
            }
            ids(l.records.iter().map(|r| &r.id))?;
            let mut total: i128 = 0;
            for r in &l.records {
                amount(r.amount)?;
                text(&r.note, 1000, false)?;
                total += r.amount as i128;
            }
            if total > l.amount as i128 {
                return Err("Repayments exceed the loan amount".into());
            }
        }
        for r in &self.reminders {
            text(&r.text, 1000, true)?;
        }
        for w in &self.wallets {
            let balance = self.balance(w);
            if balance.unsigned_abs() > MAX_AMOUNT as u64
                || self.in_base(balance, &w.currency).unsigned_abs() > MAX_AMOUNT as u64
            {
                return Err("Wallet balance exceeds the supported range".into());
            }
        }
        Ok(())
    }
    pub fn balance(&self, wallet: &Wallet) -> i64 {
        let mut sum = wallet.opening_balance;
        for t in &self.transactions {
            if t.wallet_id == wallet.id {
                sum += if t.kind == "income" {
                    t.amount
                } else {
                    -t.amount
                };
            }
            if t.to_wallet_id.as_deref() == Some(&wallet.id) {
                sum += t.to_amount.unwrap_or(0);
            }
        }
        sum
    }
    pub fn in_base(&self, minor: i64, currency: &str) -> i64 {
        let rate = *self.settings.rates.get(currency).unwrap_or(&1_000_000) as i128;
        let num = minor as i128 * rate * 10_i128.pow(digits(&self.settings.currency));
        let den = 1_000_000_i128 * 10_i128.pow(digits(currency));
        ((num + if num >= 0 { den / 2 } else { -den / 2 }) / den)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }
    pub fn pay_bill(
        &mut self,
        bill_id: &str,
        expected_due: NaiveDate,
        paid_on: NaiveDate,
    ) -> Result<String> {
        if let Some(t) = self
            .transactions
            .iter()
            .find(|t| t.bill_id.as_deref() == Some(bill_id) && t.bill_due == Some(expected_due))
        {
            return Ok(t.id.clone());
        }
        let bill = self
            .bills
            .iter_mut()
            .find(|b| b.id == bill_id)
            .ok_or("Bill not found")?;
        if bill.completed || bill.next_due != expected_due {
            return Err("Bill changed; refresh before marking it paid".into());
        }
        let tid = id();
        self.transactions.push(Transaction {
            id: tid.clone(),
            kind: bill.kind.clone(),
            wallet_id: bill.wallet_id.clone(),
            to_wallet_id: None,
            amount: bill.amount,
            to_amount: None,
            category_id: bill.category_id.clone(),
            title: bill.title.clone(),
            note: bill.note.clone(),
            date: paid_on,
            tags: vec![],
            bill_id: Some(bill.id.clone()),
            bill_due: Some(expected_due),
        });
        if bill.recurrence == "none" {
            bill.completed = true;
        } else {
            bill.next_due = next_date(bill)?;
        }
        bill.snoozed_until = None;
        self.reminders
            .retain(|r| !(r.bill_id.as_deref() == Some(bill_id) && r.due == Some(expected_due)));
        self.validate()?;
        Ok(tid)
    }
    pub fn today(&self) -> NaiveDate {
        Utc::now()
            .with_timezone(
                &self
                    .settings
                    .timezone
                    .parse::<chrono_tz::Tz>()
                    .unwrap_or(chrono_tz::UTC),
            )
            .date_naive()
    }
    pub fn reminders_at(&mut self, now: chrono::DateTime<Utc>) -> bool {
        let local = now.with_timezone(
            &self
                .settings
                .timezone
                .parse::<chrono_tz::Tz>()
                .unwrap_or(chrono_tz::UTC),
        );
        let today = local.date_naive();
        if local.hour() < self.settings.reminder_hour {
            return false;
        }
        let mut pending = vec![];
        for b in &self.bills {
            if b.paused || b.completed || b.snoozed_until.is_some_and(|d| today < d) {
                continue;
            }
            let days = (b.next_due - today).num_days();
            let trigger = if b.snoozed_until == Some(today) {
                Some(format!("snooze-{today}"))
            } else if days < 0 {
                Some("overdue".to_string())
            } else if self.settings.reminder_days.contains(&(days as u32)) {
                Some(format!("before-{days}"))
            } else {
                None
            };
            if let Some(trigger) = trigger {
                let rid = format!("bill-{}-{}-{}", b.id, b.next_due, trigger);
                if self.reminders.iter().any(|r| r.id == rid) {
                    continue;
                }
                let unit = self
                    .wallet(&b.wallet_id)
                    .map(|w| w.currency.as_str())
                    .unwrap_or(&self.settings.currency);
                let text = format!(
                    "{} · {} {} · {}",
                    b.title,
                    format_money(b.amount, unit),
                    unit,
                    if days < 0 {
                        format!("overdue since {}", b.next_due)
                    } else {
                        format!("due {}", b.next_due)
                    }
                );
                pending.push(Reminder {
                    id: rid,
                    bill_id: Some(b.id.clone()),
                    due: Some(b.next_due),
                    text,
                    created_at: now.to_rfc3339(),
                    read: false,
                    delivered: false,
                });
            }
        }
        for l in &self.loans {
            let remaining = l.amount - l.records.iter().map(|r| r.amount).sum::<i64>();
            if remaining <= 0 {
                continue;
            }
            if let Some(due) = l.due {
                let days = (due - today).num_days();
                if days > 0 && !self.settings.reminder_days.contains(&(days as u32)) {
                    continue;
                }
                let rid = format!("loan-{}-{}-{}", l.id, due, days.max(-1));
                if !self.reminders.iter().any(|r| r.id == rid) {
                    pending.push(Reminder {
                        id: rid,
                        bill_id: None,
                        due: Some(due),
                        text: format!(
                            "{} · loan {} {} · due {}",
                            l.name,
                            format_money(remaining, &l.currency),
                            l.currency,
                            due
                        ),
                        created_at: now.to_rfc3339(),
                        read: false,
                        delivered: false,
                    });
                }
            }
        }
        if self.settings.daily_entry_reminder
            && local.hour() >= 20
            && !self.transactions.iter().any(|t| t.date == today)
        {
            let rid = format!("entry-{today}");
            if !self.reminders.iter().any(|r| r.id == rid) {
                pending.push(Reminder {
                    id: rid,
                    bill_id: None,
                    due: None,
                    text: "A little check-in: have you recorded today's expenses?".into(),
                    created_at: now.to_rfc3339(),
                    read: false,
                    delivered: false,
                });
            }
        }
        let changed = !pending.is_empty();
        self.reminders.extend(pending);
        if self.reminders.len() > 1000 {
            self.reminders.drain(..self.reminders.len() - 1000);
        }
        changed
    }
}
pub fn next_date(bill: &Bill) -> Result<NaiveDate> {
    let date = match bill.recurrence.as_str() {
        "day" => bill
            .next_due
            .checked_add_days(Days::new(bill.interval as u64)),
        "week" => bill
            .next_due
            .checked_add_days(Days::new(bill.interval as u64 * 7)),
        "month" | "year" => {
            let months = bill.interval * if bill.recurrence == "year" { 12 } else { 1 };
            let first = bill
                .next_due
                .with_day(1)
                .unwrap()
                .checked_add_months(Months::new(months))
                .ok_or("Recurrence date is out of range")?;
            let end = first
                .checked_add_months(Months::new(1))
                .ok_or("Recurrence date is out of range")?
                .pred_opt()
                .unwrap();
            first.with_day(bill.anchor_day.min(end.day()))
        }
        _ => return Err("This bill does not repeat".into()),
    }
    .ok_or("Recurrence date is out of range")?;
    if date.year() > 2200 {
        return Err("Recurrence exceeds supported date range".into());
    }
    Ok(date)
}
pub fn format_money(value: i64, unit: &str) -> String {
    let factor = 10_i64.pow(digits(unit));
    if factor == 1 {
        value.to_string()
    } else {
        format!(
            "{}.{:0width$}",
            value / factor,
            value.abs() % factor,
            width = digits(unit) as usize
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Data {
        let mut d = Data::default();
        d.wallets.push(Wallet {
            id: "cash".into(),
            name: "Cash".into(),
            currency: "USD".into(),
            opening_balance: 10000,
            color: "#14CC9E".into(),
            icon: "wallet".into(),
            include_in_balance: true,
        });
        d
    }
    fn bill(due: &str) -> Bill {
        Bill {
            id: "rent".into(),
            title: "Rent".into(),
            wallet_id: "cash".into(),
            category_id: None,
            amount: 2000,
            kind: "expense".into(),
            next_due: due.parse().unwrap(),
            recurrence: "month".into(),
            interval: 1,
            anchor_day: 31,
            paused: false,
            completed: false,
            snoozed_until: None,
            note: String::new(),
        }
    }
    #[test]
    fn month_end_preserves_anchor() {
        let mut b = bill("2028-01-31");
        b.next_due = next_date(&b).unwrap();
        assert_eq!(b.next_due.to_string(), "2028-02-29");
        assert_eq!(next_date(&b).unwrap().to_string(), "2028-03-31");
    }
    #[test]
    fn settlement_is_idempotent() {
        let mut d = fixture();
        d.bills.push(bill("2026-10-31"));
        let due = "2026-10-31".parse().unwrap();
        let a = d.pay_bill("rent", due, due).unwrap();
        let b = d.pay_bill("rent", due, due).unwrap();
        assert_eq!(a, b);
        assert_eq!(d.transactions.len(), 1);
        assert_eq!(d.balance(&d.wallets[0]), 8000);
    }
    #[test]
    fn pending_bill_does_not_affect_balance() {
        let mut d = fixture();
        d.bills.push(bill("2026-10-31"));
        assert_eq!(d.balance(&d.wallets[0]), 10000);
    }
    #[test]
    fn reminders_use_timezone_and_deduplicate() {
        let mut d = fixture();
        d.settings.timezone = "Asia/Kathmandu".into();
        d.bills.push(bill("2026-10-10"));
        let now = "2026-10-10T04:00:00Z".parse().unwrap();
        assert!(d.reminders_at(now));
        assert!(!d.reminders_at(now));
        assert_eq!(d.reminders.len(), 1);
    }
    #[test]
    fn paid_bill_cancels_reminders() {
        let mut d = fixture();
        d.bills.push(bill("2026-10-10"));
        d.reminders_at("2026-10-10T10:00:00Z".parse().unwrap());
        d.pay_bill(
            "rent",
            "2026-10-10".parse().unwrap(),
            "2026-10-10".parse().unwrap(),
        )
        .unwrap();
        assert!(d.reminders.is_empty());
    }
    #[test]
    fn transfer_preserves_combined_balance() {
        let mut d = fixture();
        let mut w = d.wallets[0].clone();
        w.id = "bank".into();
        w.opening_balance = 0;
        d.wallets.push(w);
        d.transactions.push(Transaction {
            id: id(),
            kind: "transfer".into(),
            wallet_id: "cash".into(),
            to_wallet_id: Some("bank".into()),
            amount: 1234,
            to_amount: Some(1234),
            category_id: None,
            title: "Transfer".into(),
            note: "".into(),
            date: "2026-10-10".parse().unwrap(),
            tags: vec![],
            bill_id: None,
            bill_due: None,
        });
        d.validate().unwrap();
        assert_eq!(d.wallets.iter().map(|w| d.balance(w)).sum::<i64>(), 10000);
    }
    #[test]
    fn invalid_reference_is_rejected() {
        let mut d = fixture();
        let mut b = bill("2026-10-10");
        b.wallet_id = "missing".into();
        d.bills.push(b);
        assert!(d.validate().is_err());
    }
    #[test]
    fn currency_scaling_handles_zero_and_three_digits() {
        let mut d = fixture();
        d.settings.rates.insert("JPY".into(), 6500);
        d.settings.rates.insert("KWD".into(), 3_250_000);
        assert_eq!(d.in_base(1000, "JPY"), 650);
        assert_eq!(d.in_base(1000, "KWD"), 325);
    }
}
