# Piko Wallet

Piko Wallet is a personal wallet, expense tracker, and bill-deadline PWA. It recreates Ivy Wallet's clear balance header, income/expense cards, rounded account cards, planned payments, budgets, reports, and manual ledger workflow for iOS, Android, and desktop browsers.

The server is a small Rust/Axum application with a single SQLite ledger. It stores money as integer minor units, hashes passwords with Argon2, uses HttpOnly SameSite sessions, enforces optimistic revisions, validates foreign keys and amounts, and keeps financial API responses out of the service-worker cache. Planned payments remain outside actual balances until settled; recurring settlement is idempotent and timezone-aware.

Ivy Wallet is the design and behavior reference. Piko is a new web implementation and is licensed GPL-3.0-only. Read [NOTICE.md](NOTICE.md) for attribution and [LICENSE](LICENSE) for terms.

## Build and test

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
```

Run locally after building:

```sh
PIKO_ORIGIN=http://127.0.0.1:33239 \
PIKO_SETUP_TOKEN=local-setup-key \
PIKO_STATIC_DIR=static \
PIKO_DATABASE=data/piko.db \
SERVER_PORT=33239 \
./target/release/piko-wallet
```

See [DEPLOY.md](DEPLOY.md) for Botkeep and [VALIDATION.md](VALIDATION.md) for the checks run for this release.

## Included workflows

Wallets and opening balances, income/expenses/transfers, categories and tags, search and month filters, recurring or one-time planned payments, overdue views, idempotent settlement, snoozing, monthly budgets, spending reports, loan repayments, multicurrency wallets with manual exchange rates, password protection, light/dark themes, JSON backup/restore, CSV export, and Ivy ZIP/UTF-16 JSON migration are implemented.

This is a web port of Ivy's main financial workflows, not the original Android code running on a server. Native widgets, quick-settings tiles, biometric unlock, live bank integrations, full Ivy import parity, and browser push are not implemented. Ivy imports currently reject multicurrency snapshots and complex budget scopes rather than silently changing their values. Reminders appear in the app inbox and can be sent to Telegram when configured.

## Reproduce browser validation

Node and npm are needed only for development checks; production serves the static frontend from Rust.

```sh
cargo build --locked
npm ci
npx playwright install chromium webkit
npm run test:browser
```

The browser harness runs API validation first, starts a separate server with a disposable database, imports a generated demo snapshot, then runs Chromium, WebKit, and Axe checks. Set `PIKO_BINARY` to a compiled executable if it is outside `target/debug`. The fixture and temporary databases go into a separate work directory; `PIKO_TEST_WORKDIR` can override it.
