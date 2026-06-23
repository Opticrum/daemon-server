# rust-server

Opticrum REST API + background rent extraction service. Wraps the [opticrum](../opticrum) calculator crate into an HTTP server with managed wallets, SQLite persistence (Diesel ORM), and automated rent extraction / auto-match schedulers.

## Quick Start

```bash
cd fiber/rust-server
cargo build
cargo test                    # 86 tests, 0 failures (all external data mocked)

# Start the server (configure via config.toml or CLI flags)
cargo run
cargo run -- --port 9090 --log-level debug
```

The server reads `config.toml` by default. Copy and edit it — all values have sane defaults. The CKB network (testnet/mainnet) is **auto-detected** from the RPC URL, no `--network` flag needed.

> **Note:** If you have a pre-Diesel `data/opticrum.db` from an older version, delete it first (`rm data/opticrum.db`) — the legacy format is incompatible.

## Architecture

```
Client (browser / CLI / external wallet)
    │
    ▼
┌──────────────────────────────────────────────────┐
│  actix-web HTTP API (19 routes)                  │
│  /api/health, /api/wallets, /api/orders,         │
│  /api/matches, /api/fiber, /api/transactions,    │
│  /api/admin/stats, /api/admin/auto-match/config  │
└──────────┬───────────────────────────────────────┘
           │
    ┌──────▼──────┐   ┌──────────────────┐
    │  Services   │──▶│  SQLite (Diesel) │
    │  (business  │   │  wallets, orders,│
    │   logic)    │   │  matches,        │
    └──────┬──────┘   │  unsigned_txs,   │
           │          │  extraction_hist │
    ┌──────▼──────┐   └──────────────────┘
    │  Chain      │──▶ CKB RPC / Indexer
    │  Provider   │
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │   Signer    │──▶ Internal (server key) or External (JoyID / UTXOGlobal)
    └─────────────┘

Background tasks:
  • rent_extractor — auto-extracts linearly-vested rent from managed matches
  • auto_matcher  — scans on-chain orders, auto-matches against Fiber channels
```

## API Endpoints

### Health
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/health` | Liveness probe |

### Wallets
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/wallets` | Import private key |
| `GET` | `/api/wallets` | List managed wallets |
| `DELETE` | `/api/wallets/{id}` | Remove wallet |

### Orders
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/orders` | Create liquidity order |
| `GET` | `/api/orders` | List tracked orders |
| `GET` | `/api/orders/scan` | Scan chain for live orders |
| `POST` | `/api/orders/{id}/cancel` | Cancel unmatched order |
| `POST` | `/api/orders/{id}/match` | Match order with Fiber channel |

### Matches
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/matches` | List tracked matches |
| `GET` | `/api/matches/scan` | Scan chain for live matches |
| `POST` | `/api/matches/{id}/extract` | Extract rent from match |
| `POST` | `/api/matches/{id}/destroy` | Destroy exhausted match |

### Fiber Channels
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/fiber/channels` | Scan Fiber network for channels |

### External Signing
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/transactions/unsigned` | List pending unsigned transactions |
| `GET` | `/api/transactions/unsigned/{id}` | Get unsigned tx data for signing |
| `POST` | `/api/transactions/unsigned/{id}/witnesses` | Submit signed witnesses |
| `POST` | `/api/transactions/unsigned/{id}/submit` | Broadcast signed tx to chain |

### Admin
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/admin/stats` | Dashboard statistics |
| `GET` | `/api/admin/auto-match/config` | Get auto-match configuration |
| `PUT` | `/api/admin/auto-match/config` | Update auto-match configuration |

## Configuration

All settings support CLI flags (`--flag`), environment variables (`OPTICRUM_*`), and TOML config file. Priority: CLI > env > config file > defaults.

### Server
| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--config` | `OPTICRUM_CONFIG` | `config.toml` | TOML config file path |
| `--port` | `OPTICRUM_PORT` | `8080` | HTTP listen port |
| `--bind-address` | `OPTICRUM_BIND_ADDRESS` | `0.0.0.0` | Network interface |
| `--database-url` | `OPTICRUM_DATABASE_URL` | `data/opticrum.db` | SQLite database path |
| `--log-level` | `OPTICRUM_LOG_LEVEL` | `info` | Log level: trace, debug, info, warn, error |

### CKB Chain
| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--ckb-rpc-url` | `OPTICRUM_CKB_RPC_URL` | `http://localhost:8114` | CKB RPC endpoint |
| `--ckb-indexer-url` | `OPTICRUM_CKB_INDEXER_URL` | `http://localhost:8116` | CKB Indexer endpoint |
| `--fiber-rpc-url` | `OPTICRUM_FIBER_RPC_URL` | `http://localhost:8227` | Fiber network RPC |
| `--fee-rate` | `OPTICRUM_FEE_RATE` | `1000` | Tx fee in shannons/KB |

The **network** (testnet/mainnet) is auto-detected from `--ckb-rpc-url`: URLs containing `testnet`/`aggron` or port `28114` → testnet; `mainnet`/`lina` → mainnet.

### Rent Extraction
| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--scheduler-interval-secs` | `OPTICRUM_SCHEDULER_INTERVAL_SECS` | `60` | Extraction cycle interval |
| `--min-extraction-amount-shannons` | `OPTICRUM_MIN_EXTRACTION_SHANNONS` | `100000000` | Min rent to extract (1 CKB) |

### Auto-Match
| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--auto-match-enabled` | `OPTICRUM_AUTO_MATCH_ENABLED` | `false` | Enable auto-matching |
| `--auto-match-min-capacity` | `OPTICRUM_AUTO_MATCH_MIN_CAPACITY` | `10000000000` | Min order capacity (100 CKB) |
| `--auto-match-max-escrow-blocks` | `OPTICRUM_AUTO_MATCH_MAX_ESCROW_BLOCKS` | `432000` | Max escrow blocks (~30 days) |
| `--auto-match-interval-secs` | `OPTICRUM_AUTO_MATCH_INTERVAL_SECS` | `120` | Auto-match cycle interval |

## Logging

Structured logging via `tracing`. Every HTTP request is logged with method, path, status, and duration. State mutations (create/cancel/match/extract/destroy) log at `info` level with structured fields.

```bash
RUST_LOG=info cargo run     # Default: startup + state changes + errors
RUST_LOG=debug cargo run    # Also: reads, scans, scheduler skip reasons
RUST_LOG=error cargo run    # Errors only
```

Errors are automatically logged in `AppError::error_response()`, so every 4xx/5xx response is captured.

## Testing

All 86 tests use `MockChainProvider` and in-memory SQLite — no CKB node required.

```bash
cargo test                         # All 86 tests (0.3s)
cargo test --lib                   # 29 unit tests
cargo test --test db_tests         # 18 DB layer tests
cargo test --test api_tests        # 13 API endpoint tests
cargo test --test wallet_service_tests
cargo test --test order_service_tests
cargo test --test match_service_tests
cargo test --test rent_service_tests
cargo test --test scheduler_tests
cargo test --test config_tests
```

## Dependencies

| Category | Crate | Purpose |
|----------|-------|---------|
| HTTP | `actix-web` 4 | REST API server |
| ORM | `diesel` 2 | Type-safe query builder, SQLite |
| Migrations | `diesel_migrations` | Versioned schema migrations |
| CKB | `opticrum-calculator` | Transaction assembly (path dep) |
| CKB | `opticrum-protocol` | Shared types (path dep) |
| CKB | `ckb-cinnabar-calculator` | RPC client (git dep) |
| Crypto | `aes-gcm`, `secp256k1`, `sha2` | Wallet encryption + signing |
| CLI | `clap` 4, `toml` | Argument parsing, config file |
| Logging | `tracing`, `tracing-subscriber` | Structured logging |
| Async | `tokio`, `async-trait` | Async runtime |

## Project Structure

```
src/
├── main.rs              # Entry point — wires AppState, spawns schedulers
├── config.rs            # CLI/env/config file parsing (19 fields)
├── error.rs             # Unified AppError → HTTP responses + logging
├── lib.rs               # Module declarations
├── api/
│   ├── mod.rs           # AppState, RequestLogger middleware, route config
│   ├── health.rs        # Liveness probe
│   ├── wallet.rs        # Wallet CRUD handlers
│   ├── orders.rs        # Order create/list/cancel/match handlers
│   ├── matches.rs       # Match list/scan/extract/destroy handlers
│   ├── fiber.rs         # Fiber channel scan handler
│   ├── transactions.rs  # External signing flow handlers
│   └── admin.rs         # Dashboard stats + auto-match config
├── services/
│   ├── chain_provider.rs      # ChainProvider trait + MockChainProvider
│   ├── real_chain_provider.rs # Production CKB RPC implementation
│   ├── signer.rs              # Signer trait (pluggable signing)
│   ├── internal_signer.rs     # Server-stored encrypted key signing
│   ├── external_signer.rs     # Unsigned tx for external wallets
│   ├── transaction_assembler.rs # opticrum_calculator transaction pipeline
│   ├── wallet_service.rs      # Key import, derivation, encryption
│   ├── order_service.rs       # Order create/cancel logic
│   ├── match_service.rs       # Order-to-channel matching
│   ├── rent_service.rs        # Rent extraction + match destruction
│   └── crypto.rs              # AES-256-GCM encrypt/decrypt
├── db/
│   ├── mod.rs           # DbPool type, init_db, init_test_db
│   ├── schema.rs        # Diesel table! macros + migration runner
│   ├── wallets.rs       # Wallet CRUD (Diesel DSL)
│   ├── orders.rs        # Order CRUD (Diesel DSL)
│   ├── matches.rs       # Match + extraction_history CRUD (Diesel DSL)
│   └── unsigned_txs.rs  # Unsigned transaction CRUD (Diesel DSL)
└── scheduler/
    ├── mod.rs           # Spawns both background tasks
    ├── rent_extractor.rs # Auto rent extraction loop
    └── auto_matcher.rs  # Auto order matching loop

migrations/
└── 20240623000001_initial_schema/
    ├── up.sql           # Schema DDL (wallets, orders, matches, etc.)
    └── down.sql         # Rollback DDL
```
