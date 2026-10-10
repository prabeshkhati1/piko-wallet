# Piko Wallet validation

Validated on 10 October 2026 against the source in this folder. Tests used an isolated SQLite database and a local HTTP server; no personal data was used.

## Results

- Rust unit tests: **13 passed, 0 failed**. These cover exact minor-unit arithmetic, currency scaling, transfer conservation, month-end recurrence, bill settlement idempotency, timezone reminder deduplication, paid-reminder cleanup, invalid references, database restart persistence, rollback, and stale revision rejection.
- API integration: **17 checks passed**. Covered password setup, origin checks, authenticated access, invalid amount and foreign-key rejection, stale-write conflicts, income/expense/transfer balances, unpaid bill separation, loan repayments, recurring settlement, dependency-protected deletion, backup round-trip, invalid restore rollback, PWA asset responses, private database non-exposure, no-store API responses, logout, and login persistence.
- Browser flow: **19 checks passed** with Playwright. Covered all major screens, transaction search/add/edit/delete, 390 px iPhone layout, no horizontal overflow, light/dark theme, mobile add/payment forms, mobile navigation, offline shell behavior, lock flow, and zero browser JavaScript errors.
- WebKit: iPhone 13 viewport, login, navigation, and expense create/delete passed with zero JavaScript errors. This is browser-engine emulation, not a physical iPhone test.
- Accessibility and overflow: **20 screen/theme combinations** (ten screens in light and dark) passed with **0 Axe violations** and no horizontal overflow after contrast and heading-order fixes.
- JavaScript syntax: `node --check static/app.js` and `node --check static/sw.js` passed.
- Frontend currency conversion: **3 exact-arithmetic checks passed** for zero-decimal, three-decimal, and fractional exchange rates using BigInt conversion.
- Rust formatting and lint: `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed.
- Release builds: Windows x86_64 and statically linked Linux x86_64-musl compiled successfully with the locked dependencies. The Linux ELF header and program segments were inspected; no runtime interpreter is required. Linux execution remains to be checked on the server.
- PWA icon/manifest checks: 192 px, 512 px, maskable, Apple touch icon, manifest, and service worker all responded successfully. The tested service worker excludes `/api/` and financial records from caches.
- Local development-process footprint after browser checks: approximately **25 MB working set** on Windows. This is an indicative local measurement, not a Botkeep load benchmark or a production-memory guarantee.

## Screenshots

- [Desktop overview](previews/desktop.png)
- [iPhone light overview](previews/iphone-light.png)
- [iPhone dark overview](previews/iphone-dark.png)

## Remaining deployment validation

Botkeep deployment has not been performed. Before relying on reminders, verify the node's x86_64 architecture, persistent storage, always-on scheduling, assigned public port, domain routing, HTTPS, and Telegram credentials if used. The compiled Linux executable has not been run on Linux in this session. Telegram delivery was not exercised with live credentials, and browser push is not implemented in this release.
