# Piko Wallet — implementation plan and review

## Objective

An installable, mobile-first personal wallet PWA for iOS, Android, and desktop, with a Rust/SQLite server appropriate for Botkeep's 2 GB RAM, 150% CPU, and 2 GB disk allocation. Recreate Ivy Wallet's visual language and financial workflows for the web, using its repository as the source reference. Preserve GPL-3.0 and attribution for reused material. Piko is a manual ledger, not a payment processor.

## Plan

1. Inspect Ivy's real screens, typography, colors, transaction model, planned-payment behavior, and backup format. Define web equivalents for Android-specific functionality.
2. Implement a single-user Rust API and static-file server: password setup/login, authenticated sessions, SQLite persistence, wallets, categories, transactions, budgets, loans, planned bills, reports, export/import, and scheduled reminders.
3. Recreate Ivy's balance header, paired income/expense cards, rounded account/category cards, transaction rows, navigation, add-action sheet, and light/dark themes. Add bill-deadline configuration to its existing planned-payment concept. Adapt spacing to iPhone safe areas and desktop widths.
4. Add manifest, install icons, service worker, offline shell, and an explicit connectivity state. Financial writes require a connection in this version; cached authenticated financial responses must not be stored in shared service-worker caches.
5. Add persistent in-app reminders and optional Telegram delivery. Web push needs a separate configured VAPID delivery integration; do not imply push works simply because the app is installable. Reminders must survive process restarts, stop after settlement, and use timezone-aware scheduling with duplicate prevention.
6. Provide branding assets, complete source, GPL notices, reproducible Linux release workflow, and Botkeep deployment instructions. Compile elsewhere and deploy a release binary with persistent data storage.
7. Validate financial correctness, authentication, recurrence, bill settlement, SQLite restart persistence, API behavior, responsive UI, keyboard usage, PWA assets, and packaging. Record actual results and remaining deployment requirements.

## Plan review before implementation

- **Feasibility:** A small Rust service and SQLite database fit the allocation at personal-use scale. Keep attachments out of the initial version and bound backups. No claim of measured production capacity before benchmarking.
- **Ivy reuse:** Kotlin/Compose components cannot execute in a browser. Port their behavior and visual design, reuse applicable font assets, and preserve the license. Native Android widgets, quick-settings tiles, APK signing, and Android biometric APIs need platform-specific equivalents rather than direct reuse. This deliverable is a web port, not a claim of exact Android feature parity.
- **Money correctness:** Store integer minor units. Transfers are paired account movements and excluded from expense totals. Pending bills do not reduce the actual balance until paid. Settlement must be atomic and idempotent.
- **Deadline correctness:** Store recurrence in local calendar dates and a named timezone. Define month-end clamping and stop duplicate reminders. A scheduled job creates reminders, not actual bank payments.
- **Data protection:** Require a password, hash it, use HttpOnly same-site cookies, check origins for state-changing requests, rate-limit login attempts, and avoid exposing the database or configuration through static files. HTTPS is required on the deployed domain.
- **Data portability:** Provide validated Piko backups and support importing Ivy's ZIP/UTF-16 JSON backup. Preview/confirm imports in the UI and preserve foreign-key relationships. Do not describe snapshot imports as live synchronization.
- **Delivery constraints:** Botkeep public routing, HTTPS, durable storage, and always-on behavior are not yet verified. Build locally, test locally, and supply deployment instructions; do not claim hosted deployment without access.
- **Logo:** Image-generation capability is unavailable. An original vector P/wallet mark was authored directly, and PNG installation icons were exported from it. No image model or CLI was used.

## Acceptance checks

- Start from a clean database and create a wallet, category, income, expense, transfer, budget, loan, and bill through the interface/API.
- Verify arithmetic after edits/deletions and transfers; mark a bill paid twice without duplicate ledger entries.
- Verify month-end recurrence, upcoming/overdue classification, reminders, and restart persistence.
- Reject unauthenticated writes, invalid foreign keys, invalid amounts, and untrusted origins.
- Export and restore data; exercise an Ivy-format fixture, invalid files, and duplicate detection.
- Inspect mobile and desktop screenshots, light/dark themes, form behavior, and install assets.
- Run Rust formatting, compile, tests, lint where available, API integration tests, and browser checks.

## Status

Plan reviewed against the repository and hosting constraints, followed by implementation and validation. The core web port, brand assets, Linux binary, and deployment instructions are complete. Validation results and remaining deployment/feature limitations are recorded in VALIDATION.md and README.md.
