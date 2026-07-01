# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                          # Compile the project
cargo run -- --port 8080 --encryption-password your-password  # Start server
cargo test                           # All 85 tests (in-memory SQLite, no CKB node needed)
cargo test --lib                     # 27 unit tests
cargo test --test db_tests           # 18 DB layer tests
cargo test --test api_tests          # 13 API endpoint tests
cargo test --test wallet_service_tests
cargo test --test order_service_tests
cargo test --test match_service_tests
cargo test --test rent_service_tests
cargo test --test scheduler_tests
cargo test --test config_tests
cargo fmt                            # Format code
cargo clippy --all-features          # Lint (must pass)
cd web-console && npm run lint       # ESLint — zero errors AND zero warnings required
cd web-console && npm run build      # Frontend build
```

## Architecture

This is an actix-web 4 REST API + background service for the Opticrum/Fiber decentralized liquidity marketplace on CKB (Nervos). It wraps the `opticrum-calculator` crate into an HTTP server with managed wallets, SQLite persistence, and an automated rent extraction scheduler.

### Layer Stack

```
main.rs (binary entry point, wires everything)
  ├── api/          — actix-web route handlers (thin, delegate to services)
  │   ├── admin.rs, health.rs, wallet.rs, orders.rs, matches.rs
  │   ├── fiber.rs       — Fiber channel query endpoint
  │   └── transactions.rs — external signing flow endpoints
  ├── services/      — business logic, generic over ChainProvider + Signer traits
  │   ├── chain_provider.rs  — ChainProvider trait + MockChainProvider
  │   ├── real_chain_provider.rs — production CKB RPC implementation
  │   ├── signer.rs          — Signer trait for pluggable transaction signing
  │   ├── internal_signer.rs — server-stored encrypted key signing
  │   └── external_signer.rs — unsigned tx data for JoyID/UTXOGlobal/etc.
  ├── db/            — raw SQLite CRUD (rusqlite, r2d2 connection pool)
  │   └── schema.rs  — 5 tables: wallets, tracked_orders, tracked_matches,
  │                    extraction_history, unsigned_transactions
  ├── scheduler/     — background tasks (actix-rt::spawn)
  │   ├── rent_extractor.rs  — auto rent extraction loop
  │   └── auto_matcher.rs    — auto order matching loop
  ├── config.rs      — clap derive CLI/env config (19 fields)
  └── error.rs       — unified AppError enum implementing actix ResponseError
```

### Key Design Patterns

**Single source of truth for shared types**: All protocol-level types (`OrderInfo`, `MatchInfo`, `OrderArgs`, `OrderData`, `MatchArgs`, `MatchData`, `OutPoint`, `Xudt`, `AnnualYield`) are defined in the contract kernel (sibling `../opticrum/` workspace) and re-exported through `opticrum-calculator`. The server must **never** define its own copy of these types — always import from `opticrum_calculator` or `opticrum_protocol`. Server-side-only types (`CellOutput`, `ChainProvider`, DB records, API request/response types) are fine to define locally.

**AppState injection**: All handlers get `web::Data<AppState>`, which holds the DB pool, `Config`, `Arc<dyn ChainProvider>`, and `Arc<dyn Signer>`. No handler constructs its own provider — everything flows from `main.rs` wiring.

**ChainProvider trait** (`src/services/chain_provider.rs`): All chain interactions go through this trait. Methods: `get_tip_block_number`, `scan_orders`, `scan_matches`, `send_transaction`, `get_cell`, `scan_fiber_channels`. Two implementations: `MockChainProvider` (in-memory `Mutex`-based, for tests) and `RealChainProvider` (wraps `ckb_cinnabar::RpcClient`, delegates scanning to `opticrum_calculator::reader`). The trait is `Send + Sync` and stored as `Arc<dyn ChainProvider>` in `AppState`. Service functions use `P: ChainProvider + ?Sized` to accept both concrete types and trait objects.

**Signer trait** (`src/services/signer.rs`): Pluggable transaction signing. Two implementations:
- `InternalSigner` — decrypts a server-stored private key (AES-256-GCM) and signs with secp256k1. Required for auto-match.
- `ExternalSigner` — produces unsigned transaction JSON for external wallets (JoyID, UTXOGlobal). Pending signatures flow through `GET/POST /api/transactions/unsigned/*` endpoints and the `unsigned_transactions` DB table.

**Background schedulers** (`src/scheduler/`): Two independent loops spawned via `actix_rt::spawn`:
- Rent extractor — scans managed matches, auto-extracts rent above threshold.
- Auto-matcher — scans on-chain orders, filters by configurable criteria, matches against available Fiber channels. Only runs if `auto_match_enabled=true`.

**DB layer is raw SQL** (`src/db/schema.rs`): Migrations are idempotent `CREATE TABLE IF NOT EXISTS` statements run on every startup. Five tables: `wallets`, `tracked_orders`, `tracked_matches`, `extraction_history`, `unsigned_transactions`.

**Service layer is async + trait-generic**: `order_service`, `match_service`, `rent_service` all take `&P: ChainProvider + ?Sized` + `&Pool`. They build placeholder transaction hex strings (prefixed like `"create_order:..."`) and send them via the provider. Phase 6 (future) will wire real `opticrum_calculator` transaction assembly.

**Web admin panel**: Served at `/admin/` via `actix-files` from the `static/` directory. Vanilla HTML/CSS/JS calling REST API. Sections: Dashboard, Wallets, On-Chain Order Browser, Fiber Channel Browser, Matches, Auto-Match Config, External Signing Queue.

### Key Dependencies

- **opticrum-calculator** / **opticrum-protocol**: Path deps to `../opticrum/` — transaction assembly and data layouts. Not in this workspace.
- **ckb-cinnabar-calculator**: Git dep from `github.com/ashuralyk/ckb-cinnabar.git` (branch `master`) — CKB chain interaction.
- **rusqlite + r2d2**: SQLite with bundled compilation, connection pooling.
- **aes-gcm + secp256k1**: Wallet private keys encrypted at rest with AES-256-GCM. Encryption key derived from the `--encryption-password` via SHA-256.
- **actix-web 4**: HTTP framework. The server is `actix_web::main` with a single `AppState` (db pool + config) injected via `web::Data`.
- **clap 4**: CLI with derive, all flags have corresponding `OPTICRUM_*` env vars.

### Configuration

All config supports CLI flags and env vars. Notable: `--encryption-password` / `OPTICRUM_ENCRYPTION_PASSWORD` is **required** (no default). Key config groups:

- **Chain identity**: `--network` (testnet/mainnet) sets the CKB chain for contract type_id resolution. `--contract-type-id` (64-char hex) explicitly overrides the Opticrum contract's type_script args for custom/dev deployments.
- **Signing**: `--signing-mode` (internal/external) selects between server-stored key signing and external wallet signing (JoyID/UTXOGlobal). `--auto-sign-wallet-id` picks which DB wallet to use for internal signing.
- **Auto-match**: `--auto-match-enabled`, `--auto-match-min-capacity`, `--auto-match-max-escrow-blocks`, `--auto-match-interval-secs`.

Unit tests in `config.rs` clear `OPTICRUM_*` env vars before each test to avoid leakage — full integration tests for env var parsing live in `tests/config_tests.rs` to avoid race conditions with parallel test execution.

### Test Infrastructure

- `src/db/mod.rs` exposes `init_test_db()` (in-memory SQLite) — available in both test and non-test builds.
- `tests/common/mod.rs` provides shared helpers: `test_db()`, `test_private_key_hex()`, `mock_with_order()`, `mock_with_match()`, `test_cell()`, `test_app_state()`.
- All service tests are `#[actix_rt::test]` (async).
- Unit tests live inline in `src/` files under `#[cfg(test)]`; integration tests in `tests/` use `[[test]]` entries in `Cargo.toml`.
- Tests that need env var isolation use `clear_opticrum_env()` helper which removes all `OPTICRUM_*` vars.
