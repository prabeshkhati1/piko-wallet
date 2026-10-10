//! Import Ivy's ZIP/UTF-16 JSON snapshots. This is migration, not live sync.
use crate::model::*;
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use std::io::{Cursor, Read};

fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}
fn list<'a>(v: &'a Value, key: &str) -> Vec<&'a Value> {
    v[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|x| !x["isDeleted"].as_bool().unwrap_or(false))
                .collect()
        })
        .unwrap_or_default()
}
fn reference(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}
fn date(v: &Value, key: &str) -> Option<NaiveDate> {
    let value = &v[key];
    if let Some(t) = value.as_str() {
        DateTime::parse_from_rfc3339(t)
            .ok()
            .map(|t| t.date_naive())
            .or_else(|| t.get(..10).and_then(|s| s.parse().ok()))
    } else if let Some(n) = value.as_i64() {
        DateTime::<Utc>::from_timestamp(
            if n.abs() > 100_000_000_000 {
                n / 1000
            } else {
                n
            },
            0,
        )
        .map(|t| t.date_naive())
    } else {
        None
    }
}
fn money(v: &Value, key: &str, unit: &str) -> Result<i64> {
    let n = v[key]
        .as_f64()
        .or_else(|| v[key].as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| format!("Missing or invalid Ivy amount: {key}"))?;
    let value = (n * 10_f64.powi(digits(unit) as i32)).round();
    if !value.is_finite() || value.abs() > MAX_AMOUNT as f64 {
        return Err("Ivy amount exceeds supported range".into());
    }
    Ok(value as i64)
}
fn hex(v: &Value) -> String {
    v.as_i64()
        .map(|n| format!("#{:06X}", n as u32 & 0xFFFFFF))
        .unwrap_or_else(|| "#14CC9E".into())
}
fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !bytes.len().is_multiple_of(2) {
            return Err("Invalid UTF-16 backup".into());
        }
        let words: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|x| {
                if little {
                    u16::from_le_bytes([x[0], x[1]])
                } else {
                    u16::from_be_bytes([x[0], x[1]])
                }
            })
            .collect();
        String::from_utf16(&words).map_err(|_| "Invalid UTF-16 backup".into())
    } else {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| "Backup must be UTF-8 or BOM-marked UTF-16".into())
    }
}
pub fn ivy(bytes: &[u8]) -> Result<Data> {
    let text = if bytes.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid backup ZIP")?;
        if zip.len() > 10 {
            return Err("Backup ZIP contains too many files".into());
        }
        let mut data = None;
        for i in 0..zip.len() {
            let file = zip.by_index(i).map_err(|_| "Invalid backup ZIP entry")?;
            if !file.name().ends_with(".json") {
                continue;
            }
            if data.is_some() {
                return Err("Backup ZIP must contain exactly one JSON file".into());
            }
            let mut out = vec![];
            file.take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut out)
                .map_err(|_| "Cannot read backup")?;
            if out.len() > 16 * 1024 * 1024 {
                return Err("Expanded backup is too large".into());
            }
            data = Some(decode(&out)?);
        }
        data.ok_or("Backup ZIP contains no JSON file")?
    } else {
        decode(bytes)?
    };
    let root: Value = serde_json::from_str(&text).map_err(|_| "Invalid Ivy JSON backup")?;
    if !root["accounts"].is_array() || !root["transactions"].is_array() {
        return Err("This does not look like an Ivy Wallet backup".into());
    }
    let mut d = Data::default();
    d.categories.clear();
    let base = list(&root, "settings")
        .first()
        .map(|v| s(v, "currency"))
        .filter(|v| v.len() == 3)
        .unwrap_or_else(|| "USD".into());
    d.settings.currency = base.clone();
    d.settings.rates.clear();
    d.settings.rates.insert(base.clone(), 1_000_000);
    for c in list(&root, "categories") {
        d.categories.push(Category {
            id: s(c, "id"),
            name: s(c, "name"),
            color: hex(&c["color"]),
            icon: "grid".into(),
        });
    }
    for a in list(&root, "accounts") {
        let unit = reference(a, "currency").unwrap_or_else(|| base.clone());
        if unit != base {
            return Err(format!("This Ivy backup includes {unit} accounts. Set up Piko exchange rates and migrate these accounts separately; refusing an inaccurate 1:1 conversion."));
        }
        d.wallets.push(Wallet {
            id: s(a, "id"),
            name: s(a, "name"),
            currency: unit,
            opening_balance: 0,
            color: hex(&a["color"]),
            icon: "wallet".into(),
            include_in_balance: a["includeInBalance"].as_bool().unwrap_or(true),
        });
    }
    for t in list(&root, "transactions") {
        let wallet = s(t, "accountId");
        let unit = d.wallet(&wallet)?.currency.clone();
        if date(t, "dateTime").is_none() {
            continue;
        }
        let kind = s(t, "type").to_lowercase();
        let amt = money(t, "amount", &unit)?;
        if amt == 0 {
            continue;
        }
        let to_wallet = reference(t, "toAccountId");
        let to_amount = if kind == "transfer" {
            let target = d.wallet(
                to_wallet
                    .as_deref()
                    .ok_or("Ivy transfer has no destination")?,
            )?;
            Some(if t["toAmount"].is_null() {
                amt
            } else {
                money(t, "toAmount", &target.currency)?
            })
        } else {
            None
        };
        d.transactions.push(Transaction {
            id: s(t, "id"),
            kind,
            wallet_id: wallet,
            to_wallet_id: to_wallet,
            to_amount,
            amount: amt,
            category_id: reference(t, "categoryId"),
            title: reference(t, "title").unwrap_or_else(|| "Imported transaction".into()),
            note: s(t, "description"),
            date: date(t, "dateTime").unwrap(),
            tags: vec![],
            bill_id: None,
            bill_due: None,
        });
    }
    for b in list(&root, "plannedPaymentRules") {
        let wid = s(b, "accountId");
        let unit = d.wallet(&wid)?.currency.clone();
        let start = date(b, "startDate").ok_or("Ivy planned payment has no start date")?;
        let mut due = list(&root, "transactions")
            .iter()
            .filter(|t| {
                reference(t, "recurringRuleId").as_deref() == Some(&s(b, "id"))
                    && date(t, "dateTime").is_none()
            })
            .filter_map(|t| date(t, "dueDate"))
            .min()
            .unwrap_or(start);
        let one = b["oneTime"].as_bool().unwrap_or(false);
        let paid = list(&root, "transactions").iter().any(|t| {
            reference(t, "recurringRuleId").as_deref() == Some(&s(b, "id"))
                && date(t, "dateTime").is_some()
        });
        if one && paid {
            continue;
        }
        let recurrence = if one {
            "none".into()
        } else {
            s(b, "intervalType").to_lowercase()
        };
        if due.year() < 1900 {
            due = start;
        }
        d.bills.push(Bill {
            id: s(b, "id"),
            title: reference(b, "title").unwrap_or_else(|| "Imported bill".into()),
            wallet_id: wid,
            category_id: reference(b, "categoryId"),
            amount: money(b, "amount", &unit)?,
            kind: s(b, "type").to_lowercase(),
            next_due: due,
            recurrence,
            interval: b["intervalN"].as_u64().unwrap_or(1) as u32,
            anchor_day: start.day(),
            paused: false,
            completed: false,
            snoozed_until: None,
            note: s(b, "description"),
        });
    }
    for b in list(&root, "budgets") {
        let cats = s(b, "categoryIdsSerialized");
        let accounts = s(b, "accountIdsSerialized");
        if !accounts.is_empty() && accounts != "[]" {
            return Err(
                "This Ivy backup has account-scoped budgets; those require manual migration".into(),
            );
        }
        let ids: Vec<String> = if cats.starts_with('[') {
            serde_json::from_str(&cats).map_err(|_| "Invalid Ivy budget scope")?
        } else {
            cats.split(',')
                .filter(|x| !x.trim().is_empty())
                .map(|x| x.trim().to_string())
                .collect()
        };
        if ids.len() > 1 {
            return Err(
                "This Ivy backup has multi-category budgets; those require manual migration".into(),
            );
        }
        d.budgets.push(Budget {
            id: s(b, "id"),
            name: s(b, "name"),
            amount: money(b, "amount", &base)?,
            category_id: ids.first().cloned(),
        });
    }
    for l in list(&root, "loans") {
        let lid = s(l, "id");
        let mut principal = money(l, "amount", &base)?;
        let mut records = vec![];
        for r in list(&root, "loanRecords")
            .iter()
            .filter(|r| s(r, "loanId") == lid)
        {
            if r["interest"].as_bool().unwrap_or(false) {
                continue;
            }
            let amt = money(r, "amount", &base)?;
            if s(r, "loanRecordType") == "INCREASE" {
                principal = principal
                    .checked_add(amt)
                    .ok_or("Loan amount exceeds supported range")?;
            } else {
                records.push(LoanRecord {
                    id: s(r, "id"),
                    amount: amt,
                    date: date(r, "dateTime").ok_or("Invalid Ivy loan record date")?,
                    note: s(r, "note"),
                });
            }
        }
        d.loans.push(Loan {
            id: lid,
            name: s(l, "name"),
            kind: if s(l, "type") == "LEND" {
                "lent"
            } else {
                "borrowed"
            }
            .into(),
            amount: principal,
            currency: base.clone(),
            due: None,
            note: s(l, "note"),
            records,
        });
    }
    d.validate()?;
    Ok(d)
}
use chrono::Datelike;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_utf16_fixture_and_due_rule() {
        let json = r##"{"accounts":[{"id":"cash","name":"Cash","currency":"USD","color":-1}],"transactions":[{"id":"tx1","accountId":"cash","type":"EXPENSE","amount":1.23,"dateTime":"2026-10-10T08:00:00Z"}],"categories":[],"plannedPaymentRules":[{"id":"bill","accountId":"cash","type":"EXPENSE","amount":10,"title":"Rent","oneTime":true,"startDate":"2026-10-15T08:00:00Z"}]}"##;
        let bytes: Vec<u8> = [
            vec![255, 254],
            json.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat();
        let d = ivy(&bytes).unwrap();
        assert_eq!(d.transactions[0].amount, 123);
        assert_eq!(d.bills.len(), 1);
    }
    #[test]
    fn reject_unknown_file() {
        assert!(ivy(b"{}").is_err());
    }
    #[test]
    fn reject_multicurrency_guess() {
        assert!(ivy(
            br#"{"accounts":[{"id":"cash","name":"Cash","currency":"NPR"}],"transactions":[]}"#
        )
        .unwrap_err()
        .contains("inaccurate"));
    }
}
