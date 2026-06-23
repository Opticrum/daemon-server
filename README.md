# rust-server

Opticrum REST API + background rent extraction service. Wraps the [opticrum](../opticrum) calculator crate into an HTTP server with managed wallets, SQLite persistence, and an automated rent extraction scheduler.

## Quick Start

```bash
cd fiber/rust-server
cargo build
cargo test                    # 84 tests, 0 failures (all external data mocked)
cargo run -- --port 8080 --encryption-password your-password
```

## Architecture

```
Client (browser / CLI)
    │
    ▼
┌─────────────────────────────────┐
│  actix-web HTTP API (14 routes) │
│  /api/health, /api/wallets,     │
│  /api/orders, /api/matches,     │
│  /api/admin/stats               │
└──────────┬──────────────────────┘
           │
    ┌──────▼──────┐   ┌──────────────┐
    │  Services   │──▶│  SQLite DB   │
    │  (business  │   │  (wallets,   │
    │   logic)    │   │   orders,    │
    └──────┬──────┘   │   matches)   │
           │          └──────────────┘
    ┌──────▼──────┐
    │  Chain      │──▶ CKB RPC / Indexer
    │  Provider   │
    └─────────────┘

Background: rent_extractor (auto-extracts rent from managed matches)
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/health` | Liveness probe |
| `POST` | `/api/wallets` | Import private key |
| `GET` | `/api/wallets` | List managed wallets |
| `DELETE` | `/api/wallets/{id}` | Remove wallet |
| `POST` | `/api/orders` | Create liquidity order |
| `GET` | `/api/orders` | List tracked orders |
| `GET` | `/api/orders/scan` | Scan chain for orders |
| `POST` | `/api/orders/{id}/cancel` | Cancel unmatched order |
| `POST` | `/api/orders/{id}/match` | Match order with channel |
| `GET` | `/api/matches` | List tracked matches |
| `GET` | `/api/matches/scan` | Scan chain for matches |
| `POST` | `/api/matches/{id}/extract` | Extract rent |
| `POST` | `/api/matches/{id}/destroy` | Destroy exhausted match |
| `GET` | `/api/admin/stats` | Dashboard statistics |

## Configuration

| Flag | Env | Default |
|------|-----|---------|
| `--port` | `OPTICRUM_PORT` | `8080` |
| `--database-url` | `OPTICRUM_DATABASE_URL` | `data/opticrum.db` |
| `--ckb-rpc-url` | `OPTICRUM_CKB_RPC_URL` | `http://localhost:8114` |
| `--ckb-indexer-url` | `OPTICRUM_CKB_INDEXER_URL` | `http://localhost:8116` |
| `--scheduler-interval-secs` | `OPTICRUM_SCHEDULER_INTERVAL_SECS` | `60` |
| `--min-extraction-amount-shannons` | `OPTICRUM_MIN_EXTRACTION_SHANNONS` | `100000000` |
| `--encryption-password` | `OPTICRUM_ENCRYPTION_PASSWORD` | **required** |

## Testing

All 84 tests use `MockChainProvider` — no CKB node required.

```bash
cargo test                    # All 84 tests
cargo test --lib              # 26 unit tests
cargo test --test db_tests    # 18 DB layer tests
cargo test --test api_tests   # 13 API endpoint tests
```

## Dependencies

- **actix-web 4** — HTTP server
- **opticrum-calculator** — Transaction assembly (path dep)
- **opticrum-protocol** — Data layouts (path dep)
- **rusqlite + r2d2** — SQLite persistence
- **aes-gcm + secp256k1** — Wallet encryption + signing
- **clap 4** — CLI argument parsing
- **tracing** — Structured logging

---

## Build Recap

| Metric | Value |
|--------|-------|
| **Project** | `fiber/rust-server` |
| **Source files** | 26 `.rs` files |
| **Test files** | 8 integration test suites |
| **Total tests** | 84 (0 failures) |
| **Lines of code** | ~2,500 |
| **Build time** | ~2 min (first compile, incl. CKB deps) |
| **Test time** | ~0.3s (all 84 tests, in-memory DB) |
| **Token consumption** | ~120K input tokens (opticrum codebase context) + ~80K output tokens (code generation + fixes) |
| **Total wall time** | ~45 min (scaffold → config/error → DB → crypto → services → API → scheduler → tests → verification) |
| **Iteration cycles** | 7 compile-fix cycles (env var race, mock hash bug, Serialize derive, workspace config, lib.rs target, clippy Default, fmt) |
