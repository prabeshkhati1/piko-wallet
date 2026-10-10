# Deploy Piko Wallet on Botkeep

Botkeep's Rust image starts in `/home/container` and runs the configured startup command. The previous crash happened because that directory did not contain a Rust project. Upload the **Linux release binary and `static/` directory**, rather than building the app on the small server.

## Files

```text
/home/container/piko-wallet       # executable built for Linux x86_64
/home/container/static/            # copy this repository's static directory
/home/container/data/              # persistent database directory
```

The release binary must be built for the server's Linux architecture. From a Linux CI runner or Linux machine, run:

```sh
cargo build --release --locked
cp target/release/piko-wallet /home/container/piko-wallet
```

The supplied `piko-wallet-linux-x86_64.zip` contains a cross-compiled, statically linked Linux x86_64 binary and the static app files. Its ELF architecture and absence of a runtime interpreter were checked. Core behavior was tested with the Windows build from the same source; the Linux binary has not been executed in this Windows session. Confirm your Botkeep node uses x86_64 before uploading. A reproducible Linux build workflow is included in `.github/workflows/release-linux.yml`.

## Environment

Set the Pterodactyl startup command to:

```sh
chmod +x ./piko-wallet && ./piko-wallet
```

Set these variables in Botkeep's **Environment** tab:

```text
SERVER_PORT=<the port Botkeep assigned>
PIKO_HOST=0.0.0.0
PIKO_ORIGIN=https://wallet.example.com
PIKO_SETUP_TOKEN=<long random one-time setup value>
PIKO_DATABASE=/home/container/data/piko.db
PIKO_STATIC_DIR=/home/container/static
```

Use a domain routed to the assigned Botkeep port and HTTPS. The first browser visit uses `PIKO_SETUP_TOKEN` only when creating the wallet password. Retain a setup value in the server environment (the application requires it on HTTPS startup); setup cannot overwrite an existing password. Keep the database directory persistent across redeployments.

Optional Telegram delivery:

```text
TELEGRAM_BOT_TOKEN=<bot token>
TELEGRAM_CHAT_ID=<private chat id>
```

The app always stores reminders in its inbox. Telegram is an optional server-side delivery channel. Browser push is intentionally not enabled in this release; iOS still supports installation as a PWA, but deadline reminders require opening Piko or Telegram.

## Backups

Use Settings → Export backup regularly. The database is SQLite WAL-backed. Keep only bounded backup copies under the 2 GB disk allocation, and protect the exported JSON as financial data. Do not expose `/data/` through the static directory.

## First start

1. Extract the release ZIP into `/home/container` so `piko-wallet` and `static/` are at the root. The application creates `data/` on its first start.
2. Set the environment values and startup command.
3. Start the server and open the HTTPS domain.
4. Enter the setup key and create a password of at least 12 characters.
5. Create wallets, set exchange rates before using foreign currencies, then add transactions and planned payments.
6. Confirm an export and restore before relying on the service.

## Resource expectations

The service uses SQLite and has no worker pool or image storage. It is designed for one household/personal ledger and the stated 2 GB RAM, 150% CPU, and 2 GB storage allocation. Actual Botkeep limits, persistent volume behavior, public routing, and HTTPS must be verified in the panel before deployment.
