# Opticrum Rust Server

REST API + background service for the [Opticrum](https://github.com/ashuralyk/opticrum) decentralized liquidity marketplace on [CKB](https://github.com/nervosnetwork/ckb) (Nervos Network). Wraps the `opticrum-calculator` crate into an HTTP server with HD wallet key management, SQLite persistence, an in-memory chain cache, and automated rent extraction / auto-match schedulers. Ships with a **Vue 3 web admin console** at `/admin`.

## Table of Contents

- [What is Opticrum?](#what-is-opticrum)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Running the Server](#running-the-server)
- [Configuration](#configuration)
- [Web Admin Console](#web-admin-console)
- [API Reference](#api-reference)
- [HD Wallet & Signing](#hd-wallet--signing)
- [Background Schedulers](#background-schedulers)
- [Chain Cache](#chain-cache)
- [Runtime Configuration](#runtime-configuration)
- [Testing](#testing)
- [Project Structure](#project-structure)
- [Key Dependencies](#key-dependencies)

## What is Opticrum?

Opticrum is a decentralized liquidity marketplace built on CKB's RGB++ protocol. Liquidity providers (LPs) create **orders** — programmable CKB cells that escrow capital with defined yield curves. Market makers match these orders against **Fiber Network** (CKB's Lightning Network) payment channels, earning linearly-vested **rent** that can be extracted on-chain. The system is non-custodial: funds are always locked in on-chain cells, and the server only assembles and signs transactions; it never holds user funds directly.

This server is the **off-chain coordinator** — it scans the chain for live orders and matches, maintains a cache, provides an HTTP API for the web console, and runs background tasks that automate rent extraction and order matching.

## Quick Start

### Prerequisites

- **Rust** 1.78+ (install via [rustup](https://rustup.rs))
- **Node.js** 20+ (for the web console build)
- **Access to CKB nodes**: an RPC endpoint, an Indexer endpoint, and a Fiber Network node. For testnet development you can use the public endpoints; for production, run your own.

### One-shot build & test

```bash
cd fiber/rust-server

# Backend
cargo build
cargo test                    # 135 tests, all pass (in-memory SQLite, no chain node needed)

# Frontend (Vue 3 SPA served at /admin)
cd web-console && npm install && npm run build && cd ..
```

### Start the server

```bash
# Minimal — uses defaults (localhost CKB nodes, port 8080)
cargo run

# With a config file (recommended)
cargo run -- --config config.toml

# CLI overrides for quick iteration
cargo run -- --port 9090 --log-level debug --ckb-rpc-url https://testnet.ckb.dev
```

The server reads `config.toml` by default. Copy the bundled `config.toml` and edit it — every field has a sensible default. The CKB network (testnet/mainnet) is **auto-detected** from the RPC URL at startup.

Open `http://localhost:8080/admin` for the web console.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Web Admin Console                        │
│              Vue 3 SPA (static/ → /admin)                   │
│    Dashboard │ Wallets │ Orders │ Matches │ Channels │ Settings │
└────────────────────────┬────────────────────────────────────┘
                         │ HTTP (REST + JSON)
┌────────────────────────▼────────────────────────────────────┐
│              actix-web 4 HTTP API (~50 routes)              │
│                                                             │
│  /api/health          /api/console/wallets/*                │
│  /api/wallets         /api/console/orders/*                 │
│  /api/orders/*        /api/console/matches/*                │
│  /api/matches/*       /api/console/channels/*               │
│  /api/fiber/channels  /api/console/scheduler/*              │
│  /api/admin/*         /api/console/runtime-config/*         │
└────────┬────────────────────────────────────────────────────┘
         │
┌────────▼───────────────────────────────────────────────────┐
│                      AppState                               │
│                                                             │
│  ┌──────────────┐  ┌────────────────┐  ┌─────────────────┐ │
│  │  DbPool      │  │  RuntimeConfig │  │  ChainCache     │ │
│  │  (SQLite)    │  │  (Arc<RwLock>) │  │  (in-memory)    │ │
│  └──────────────┘  └────────────────┘  └─────────────────┘ │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  ChainProvider (trait object)                        │   │
│  │  ┌────────────────────┐  ┌────────────────────────┐  │   │
│  │  │ CachedChainProvider│  │ RealChainProvider       │  │   │
│  │  │ (transparent cache)│─▶│ (CKB RPC + Indexer +    │  │   │
│  │  └────────────────────┘  │  Fiber RPC)             │  │   │
│  │                          └────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌────────────────┐  ┌──────────────────────────────────┐   │
│  │ HdWalletSigner │  │ TransactionAssembler             │   │
│  │ (BIP39/BIP32)  │  │ (opticrum-calculator tx pipeline)│   │
│  └────────────────┘  └──────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Background Tasks                            │
│                                                              │
│  chain_indexer  — scans chain every N seconds, refreshes     │
│                   the in-memory cache (orders/matches/        │
│                   channels), pushes events to console        │
│                                                              │
│  rent_extractor — auto-extracts linearly-vested rent from    │
│                   managed matches above dust threshold       │
│                                                              │
│  auto_matcher   — scans on-chain orders, filters by          │
│                   capacity/escrow criteria, matches          │
│                   against available Fiber channels           │
└─────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

| Layer | Role |
|-------|------|
| **api/** | Thin actix-web route handlers. Extract params, call services, return JSON. No business logic. |
| **services/** | Core business logic. Generic over `ChainProvider` + `Signer` traits so everything is testable with mocks. |
| **db/** | Diesel ORM CRUD. Five tables via `diesel_migrations`. |
| **scheduler/** | Background loops spawned via `actix_rt::spawn`. Chain indexer, rent extractor, auto-matcher. |
| **fiber/** | Vendored Fiber JSON-RPC client (subset needed for channel queries). |

### Key Design Principles

**Trait-based abstraction.** `ChainProvider` and `Signer` traits allow swapping implementations. Tests use `MockChainProvider` (in-memory `Mutex`-based); production uses `RealChainProvider` (CKB RPC). The `CachedChainProvider` wraps any provider with a transparent read-through cache.

**Single source of truth for protocol types.** Order/Match/OutPoint/Xudt types all come from `opticrum-calculator` / `opticrum-protocol`. The server never defines its own copies.

**Dependency injection via AppState.** All handlers receive `web::Data<AppState>`, which holds the DB pool, chain provider, signer, cache, and config. Nothing is constructed inside a handler.

**Runtime-configurable.** Most operational settings (fee rate, extraction thresholds, auto-match params) can be changed at runtime via `PUT /api/console/runtime-config` — no restart needed.

**HD wallet (BIP39/BIP32).** A single mnemonic seed derives a tree of child keys (BIP44 `m/44'/309'/0'/0/i`). The keystore is AES-256-GCM encrypted and unlocked in the admin panel with a password. Signing keys are held in memory only while unlocked.

## Running the Server

### Step-by-step

**1. Configure `config.toml`**

```bash
cp config.toml config.local.toml   # edit to taste
```

At minimum, point `ckb_rpc_url` and `ckb_indexer_url` at your CKB nodes. For testnet:

```toml
ckb_rpc_url = "https://testnet.ckb.dev"
ckb_indexer_url = "https://testnet.ckb.dev/indexer"
fiber_rpc_url = "http://localhost:8227"
```

**2. Start the server**

```bash
cargo run -- --config config.local.toml
```

You'll see output like:

```
INFO opticrum_server: Chain connected tip=15234000 network=testnet
INFO opticrum_server: Opticrum Server starting version=0.1.0 port=8080
INFO opticrum_server: Server ready address=http://0.0.0.0:8080/admin
INFO opticrum_server: Chain indexer started
INFO opticrum_server: Rent extractor started
INFO opticrum_server: Auto-matcher started
```

**3. Open the web console**

Navigate to `http://localhost:8080/admin`. The console is a single-page Vue 3 app that calls the REST API.

**4. Create or import an HD wallet**

In the admin console → **Wallets** tab → **Create HD Wallet**. This generates a BIP39 mnemonic and stores it encrypted in `data/keystore.json`. The mnemonic is shown once — back it up.

Alternatively, import an existing mnemonic via the **Import Mnemonic** button.

**5. Unlock the wallet**

Enter your password in the Wallets tab to decrypt and load the signing keys. Without unlocking, the server can read chain data but cannot sign transactions (rent extraction, auto-match).

**6. (Optional) Auto-unlock on startup**

Add `hd_wallet_password` to `config.toml`:

```toml
hd_wallet_password = "your-password"
```

The server will auto-decrypt the keystore on boot. Use this for unattended deployments.

### Docker (coming soon)

A `Dockerfile` is planned — for now, build directly on the host or use `cargo build --release` for a standalone binary.

## Configuration

All settings accept three sources, resolved in priority order: **CLI flags → environment variables → config.toml → defaults**.

### Server

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--config` | `OPTICRUM_CONFIG` | `config.toml` | TOML config file path |
| `--port` | `OPTICRUM_PORT` | `8080` | HTTP listen port |
| `--bind-address` | `OPTICRUM_BIND_ADDRESS` | `0.0.0.0` | Network interface |
| `--database-url` | `OPTICRUM_DATABASE_URL` | `data/opticrum.db` | SQLite database path |
| `--keystore-path` | `OPTICRUM_KEYSTORE_PATH` | `data/keystore.json` | HD wallet keystore file |
| `--hd-wallet-password` | `OPTICRUM_HD_WALLET_PASSWORD` | _(none)_ | Auto-unlock password |
| `--log-level` | `OPTICRUM_LOG_LEVEL` | `info` | trace, debug, info, warn, error |

### CKB Chain

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--ckb-rpc-url` | `OPTICRUM_CKB_RPC_URL` | `http://localhost:8114` | CKB RPC endpoint |
| `--ckb-indexer-url` | `OPTICRUM_CKB_INDEXER_URL` | `http://localhost:8116` | CKB Indexer endpoint |
| `--fiber-rpc-url` | `OPTICRUM_FIBER_RPC_URL` | `http://localhost:8227` | Fiber Network RPC |
| `--fee-rate` | `OPTICRUM_FEE_RATE` | `1000` | Tx fee in shannons/KB |

Network (testnet/mainnet) is **auto-detected** from the RPC URL: URLs containing `testnet`/`aggron` or port `28114` → testnet; `mainnet`/`lina` → mainnet. If detection fails, the server queries the chain's `chain_info` RPC at startup and falls back to URL-based heuristics.

### Rent Extraction

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--rent-extraction-enabled` | `OPTICRUM_RENT_EXTRACTION_ENABLED` | `true` | Enable the extraction scheduler |
| `--scheduler-interval-secs` | `OPTICRUM_SCHEDULER_INTERVAL_SECS` | `60` | Seconds between extraction cycles |
| `--min-extraction-amount-shannons` | `OPTICRUM_MIN_EXTRACTION_SHANNONS` | `100000000` | Dust threshold (1 CKB) |

### Auto-Match

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--auto-match-enabled` | `OPTICRUM_AUTO_MATCH_ENABLED` | `false` | Enable auto-matching |
| `--auto-match-min-capacity` | `OPTICRUM_AUTO_MATCH_MIN_CAPACITY` | `10000000000` | Min order capacity (100 CKB) |
| `--auto-match-max-escrow-blocks` | `OPTICRUM_AUTO_MATCH_MAX_ESCROW_BLOCKS` | `432000` | Max escrow blocks (~30 days) |
| `--auto-match-interval-secs` | `OPTICRUM_AUTO_MATCH_INTERVAL_SECS` | `120` | Seconds between match cycles |

### Chain Cache

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--chain-cache-enabled` | `OPTICRUM_CHAIN_CACHE_ENABLED` | `true` | Enable background cache refresh |
| `--chain-cache-interval-secs` | `OPTICRUM_CHAIN_CACHE_INTERVAL_SECS` | `30` | Cache refresh interval |

### Example config.toml

```toml
port = 8080
bind_address = "0.0.0.0"
database_url = "data/opticrum.db"
keystore_path = "data/keystore.json"

ckb_rpc_url = "https://testnet.ckb.dev"
ckb_indexer_url = "https://testnet.ckb.dev/indexer"
fiber_rpc_url = "http://localhost:8227"

fee_rate = 2000
rent_extraction_enabled = false
scheduler_interval_secs = 3600
min_extraction_amount_shannons = 1000000000   # 10 CKB

auto_match_enabled = false
auto_match_min_capacity = 100000000000        # 1000 CKB
auto_match_max_escrow_blocks = 432000
auto_match_interval_secs = 300

chain_cache_enabled = true
chain_cache_interval_secs = 30

log_level = "info"
```

## Web Admin Console

A Vue 3 + TypeScript SPA built with Vite, served from `static/` at `/admin`. It provides a full management interface:

| View | What it does |
|------|-------------|
| **Dashboard** | Real-time scheduler status, chain tip, cache stats, recent events |
| **Wallets** | Create/import HD wallets, unlock/lock keystore, view balances, derive new addresses, reveal mnemonic |
| **Orders** | Browse on-chain liquidity orders, check match readiness, create Fiber channels, manually match |
| **Matches** | View active matches, extraction history, trigger rent extraction or destroy exhausted matches |
| **Channels** | Browse Fiber Network channels, view channel-match associations, close channels |
| **Settings** | In-page console showing scheduler cycles, events, and runtime config editor |

The console communicates exclusively through the `/api/console/*` endpoints. All state mutations require the wallet to be unlocked.

### Building the console

```bash
cd web-console
npm install
npm run build       # outputs to ../static/
```

The build output is checked into the repo, so `cargo run` works immediately. Rebuild only when you change the frontend.

## API Reference

### Public API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/health` | Liveness probe |
| `GET` | `/api/wallets` | List managed wallet addresses |
| `POST` | `/api/wallets` | Import a private key (legacy, prefer HD wallet) |
| `DELETE` | `/api/wallets/{id}` | Remove a wallet |
| `GET` | `/api/orders/scan` | Scan chain for live orders |
| `POST` | `/api/orders/{tx_hash}/match` | Match an order with a Fiber channel |
| `GET` | `/api/matches` | List tracked matches |
| `GET` | `/api/matches/scan` | Scan chain for live matches |
| `POST` | `/api/matches/{tx_hash}/{output_index}/extract` | Extract rent from a match |
| `POST` | `/api/matches/{tx_hash}/{output_index}/destroy` | Destroy an exhausted match |
| `GET` | `/api/fiber/channels` | List Fiber channels |
| `GET` | `/api/admin/stats` | Dashboard statistics |
| `GET` | `/api/admin/auto-match/config` | Get auto-match config |
| `PUT` | `/api/admin/auto-match/config` | Update auto-match config |

### Console API (web admin panel)

#### Wallets & Authentication

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/wallets` | List all wallets (HD + legacy) |
| `POST` | `/api/console/wallets` | Import legacy private key |
| `POST` | `/api/console/wallets/create-hd` | Create new HD wallet (generates mnemonic) |
| `POST` | `/api/console/wallets/import-mnemonic` | Import existing BIP39 mnemonic |
| `POST` | `/api/console/wallets/unlock` | Unlock keystore with password |
| `POST` | `/api/console/wallets/lock` | Lock wallet (clear keys from memory) |
| `GET` | `/api/console/wallets/session` | Check unlock session status |
| `GET` | `/api/console/wallets/hd-status` | HD wallet status (locked/unlocked, address count) |
| `GET` | `/api/console/wallets/balance` | Total balance across all HD addresses |
| `GET` | `/api/console/wallets/balances` | Per-address balances |
| `POST` | `/api/console/wallets/derive-more` | Derive additional HD addresses |
| `POST` | `/api/console/wallets/refresh-hd` | Re-scan HD addresses from chain |
| `POST` | `/api/console/wallets/reveal-mnemonic` | Reveal mnemonic (requires password) |
| `DELETE` | `/api/console/wallets/delete-hd` | Delete HD wallet and keystore |
| `DELETE` | `/api/console/wallets/{id}` | Delete individual wallet |

#### Orders & Matching

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/orders` | Scan orders with detailed info |
| `GET` | `/api/console/orders/{tx_hash}/match-readiness` | Check if an order can be matched |
| `POST` | `/api/console/orders/{tx_hash}/create-channel` | Create Fiber channel for an order |
| `POST` | `/api/console/orders/{tx_hash}/match` | Execute match transaction |

#### Matches

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/matches` | List all matches with extraction history |
| `GET` | `/api/console/matches/{tx_hash}/{output_index}` | Match detail |
| `POST` | `/api/console/matches/{tx_hash}/{output_index}/extract` | Extract rent |
| `POST` | `/api/console/matches/{tx_hash}/{output_index}/destroy` | Destroy match |

#### Channels

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/channels` | All channels (Fiber + match metadata) |
| `GET` | `/api/console/channels-only` | Raw Fiber channels only |
| `GET` | `/api/console/channel-matches` | Channel-match associations |
| `POST` | `/api/console/channels/{channel_id}/close` | Close a channel |
| `DELETE` | `/api/console/channels/{channel_id}` | Remove channel from local DB |

#### System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/dashboard` | Aggregated dashboard data |
| `GET` | `/api/console/scheduler/status` | Scheduler cycle history & events |
| `GET` | `/api/console/chain-cache/status` | Cache freshness & hit rate |
| `POST` | `/api/console/chain-cache/refresh` | Force immediate cache refresh |
| `GET` | `/api/console/server-info` | Server version, network, tip block |
| `GET` | `/api/console/runtime-config` | Current runtime settings |
| `PUT` | `/api/console/runtime-config` | Update runtime settings (partial) |
| `POST` | `/api/console/runtime-config/reset` | Reset to config.toml values |
| `GET` | `/api/console/fiber-node-info` | Fiber node metadata |
| `GET` | `/api/console/signer/wallets` | Signer-available addresses |

#### Peer Management

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/console/peers/check/{pubkey}` | Check if peer is connected |
| `POST` | `/api/console/peers/connect` | Connect to a peer by pubkey + address |

### Error Responses

All errors follow a uniform JSON shape:

```json
{
  "error": "not_found | bad_request | wallet_error | chain_error | internal_error",
  "message": "Human-readable description"
}
```

HTTP status codes: `400` for client errors, `404` for not found, `502` for chain failures, `500` for internal errors.

## HD Wallet & Signing

The server uses a **BIP39 → BIP32 → BIP44** HD wallet:

```
Mnemonic (24 words, BIP39)
  └── Master seed
       └── BIP32 root key
            └── m/44'/309'/0'/0/i   (BIP44 for CKB)
                 ├── 0: first address
                 ├── 1: second address
                 └── ...
```

- **Keystore**: The mnemonic is encrypted with AES-256-GCM (key = SHA-256 of user password) and persisted to `data/keystore.json`.
- **In-memory only**: Decrypted child private keys exist only in the `HdWalletSigner`'s `Mutex<Vec<>>`. Locking the wallet clears them.
- **Session**: Unlock creates a 1-hour HttpOnly session cookie. The console uses this to gate write operations.
- **Derivation**: New addresses are derived on demand via the **Derive More** button. Each derivation creates the next BIP44 index and stores the address + encrypted key pair in SQLite.
- **Balance**: The server scans all HD addresses against the CKB Indexer and shows aggregate + per-address balances.

### Unlock Flow

```
Browser                          Server
  │                                │
  │  POST /api/console/wallets/unlock
  │  { password: "..." }           │
  │ ─────────────────────────────> │
  │                                │ reads keystore.json
  │                                │ decrypts with SHA-256(password)
  │                                │ derives all child keys
  │                                │ loads into HdWalletSigner
  │                                │ creates session cookie
  │  ←───────────────────────────  │
  │  Set-Cookie: session=...       │
```

## Background Schedulers

Three background loops run in the server process, spawned via `actix_rt::spawn`:

### Chain Indexer

Refreshes the in-memory `ChainCache` on a configurable interval (default 30s). The cache holds:
- All live on-chain **orders** (Order cells)
- All live on-chain **matches** (Match cells)
- All **Fiber channels** from the connected Fiber node
- Freshness metadata (last scan time, tip block)

API reads hit the cache first; writes (extract, destroy, match) invalidate affected entries so the next read re-scans. The cache is observable via `/api/console/chain-cache/status`.

### Rent Extractor

Scans managed matches, computes linearly-vested rent since the last extraction, and submits extraction transactions when the accrued amount exceeds `min_extraction_amount_shannons`. Each successful extraction creates a record in the `extraction_history` table.

**Rent model**: When a match is created, rent vests linearly over its lock period. The extractor periodically claims the vested portion. A match can be extracted multiple times; when fully exhausted, it can be destroyed to recover the remaining CKB capacity.

### Auto-Matcher

Scans on-chain orders, filters by:
- `capacity >= auto_match_min_capacity`
- `escrow_blocks <= auto_match_max_escrow_blocks`
- Not already matched
- Not owned by the server's own Fiber node pubkey

For each eligible order, it attempts to match against an available Fiber channel. The match creates a Match cell on-chain and records it in SQLite for the rent extractor to manage.

**Safety**: Auto-match is disabled by default. Enable it explicitly and ensure the wallet is unlocked.

## Chain Cache

The `ChainCache` is an in-memory read-through cache that wraps the real chain provider. It:

- **Serves reads instantly** from memory (no RPC call) for order/order-list/channel queries
- **Refreshes in the background** via the chain indexer, decoupling scan latency from API response time
- **Invalidates on mutation** — after extract/destroy/match, the affected entries are evicted so the next read fetches fresh data
- **Reports metrics** — cache hit rate, last refresh time, entry counts — via the console API

Disable with `chain_cache_enabled = false` if you prefer always-fresh RPC queries at the cost of higher latency.

## Runtime Configuration

Most operational parameters can be changed without restarting the server via `PUT /api/console/runtime-config`:

```json
{
  "fee_rate": 2000,
  "rent_extraction_enabled": true,
  "scheduler_interval_secs": 300,
  "min_extraction_amount_shannons": 500000000,
  "auto_match_enabled": true,
  "auto_match_min_capacity": 50000000000,
  "auto_match_max_escrow_blocks": 216000,
  "auto_match_interval_secs": 60,
  "automation_signer_address": "ckt1qyq...",
  "chain_cache_enabled": true,
  "chain_cache_interval_secs": 15
}
```

The update is **partial** — only send the fields you want to change. Reset to config.toml defaults with `POST /api/console/runtime-config/reset`. URL fields (`ckb_rpc_url`, `ckb_indexer_url`, `fiber_rpc_url`) are editable at runtime but **require a restart** to take effect since the chain provider is initialized once at startup.

## Testing

All 135 tests use `MockChainProvider` (in-memory `Mutex`-backed fake) and in-memory SQLite — **no CKB node or Fiber node required**. Tests complete in under 0.5 seconds.

```bash
cargo test                              # All 135 tests
cargo test --lib                        # 84 unit tests
cargo test --test db_tests              # 11 DB layer tests
cargo test --test wallet_service_tests  # 3 wallet service tests
cargo test --test match_service_tests   # 4 match service tests
cargo test --test rent_service_tests    # 10 rent service tests
cargo test --test scheduler_tests       # 2 scheduler tests
cargo test --test config_tests          # 2 config tests
cargo test --test api_tests             # 5 API integration tests
cargo test --test hd_wallet_tests       # 14 HD wallet tests
```

### Test Infrastructure

- `src/db/mod.rs` exposes `init_test_db()` — in-memory SQLite available in all build profiles.
- `tests/common/mod.rs` provides `test_db()`, `test_private_key_hex()`, `mock_with_order()`, `mock_with_match()`, `test_cell()`, `test_app_state()`.
- Unit tests live inline under `#[cfg(test)]`; integration tests in `tests/` use `[[test]]` entries in `Cargo.toml`.
- `MockChainProvider` has methods like `add_order()`, `add_match()`, `set_tip_block()` to set up test scenarios deterministically.

### Linting

```bash
cargo clippy --all-features      # Must pass with zero warnings
cargo fmt --check                # Must be clean
cd web-console && npm run lint   # ESLint — zero errors AND zero warnings
```

## Project Structure

```
rust-server/
├── config.toml                  # Bundled config file (edit for your setup)
├── Cargo.toml
├── CLAUDE.md                    # AI assistant guidance
├── README.md
│
├── src/
│   ├── main.rs                  # Binary entry point — wires AppState, spawns schedulers
│   ├── lib.rs                   # Library root — re-exports all modules
│   ├── config.rs                # CLI/env/config file parsing (22 fields, merge logic)
│   ├── error.rs                 # Unified AppError → HTTP status codes + JSON body
│   │
│   ├── api/
│   │   ├── mod.rs               # AppState struct, configure_routes(), RequestLogger middleware
│   │   ├── health.rs            # GET /api/health
│   │   ├── wallet.rs            # Wallet CRUD (legacy single-key import)
│   │   ├── orders.rs            # Order scan + manual match
│   │   ├── matches.rs           # Match list/scan + extract/destroy
│   │   ├── fiber.rs             # Fiber channel list
│   │   ├── transactions.rs      # External signing endpoints
│   │   ├── admin.rs             # Dashboard stats + auto-match config
│   │   └── console/
│   │       └── mod.rs           # ~40 console routes (gateway, wallets, orders, matches,
│   │                            #   channels, scheduler, chain-cache, runtime-config, peers)
│   │
│   ├── services/
│   │   ├── mod.rs               # Module declarations + backward-compat re-exports
│   │   ├── match_service.rs     # Order-to-channel matching logic
│   │   ├── rent_service.rs      # Rent extraction + match destruction
│   │   ├── transaction_assembler.rs  # opticrum-calculator tx assembly pipeline
│   │   ├── runtime_config.rs    # Mutable-at-runtime settings (Arc<RwLock<>>)
│   │   ├── chain/
│   │   │   ├── mod.rs           # Chain sub-module declarations
│   │   │   ├── chain_provider.rs      # ChainProvider trait + MockChainProvider
│   │   │   ├── real_chain_provider.rs # Production CKB RPC implementation
│   │   │   ├── cached_chain_provider.rs # Transparent read-through cache wrapper
│   │   │   └── chain_cache.rs         # In-memory cache store (orders, matches, channels)
│   │   ├── wallet/
│   │   │   ├── mod.rs           # Wallet sub-module declarations
│   │   │   ├── wallet_service.rs      # Key import, derivation orchestration
│   │   │   ├── hd_wallet.rs           # BIP39/BIP32/BIP44 derivation
│   │   │   ├── keystore.rs            # AES-256-GCM encrypted mnemonic persistence
│   │   │   ├── wallet_session.rs      # HttpOnly cookie session management
│   │   │   ├── crypto.rs              # AES-256-GCM encrypt/decrypt primitives
│   │   │   ├── address.rs             # CKB address parsing/generation
│   │   │   ├── signer.rs              # Signer trait (pluggable transaction signing)
│   │   │   ├── internal_signer.rs     # Legacy single-key signer
│   │   │   └── hd_wallet_signer.rs    # In-process signer using decrypted HD child keys
│   │   └── console/
│   │       ├── mod.rs           # Console service module
│   │       ├── gateway_service.rs    # Dashboard aggregation logic
│   │       └── scheduler_state.rs    # Cycle history + event log for console
│   │
│   ├── db/
│   │   ├── mod.rs               # DbPool type alias, init_db(), init_test_db()
│   │   ├── schema.rs            # Diesel table! macros + embed_migrations!()
│   │   ├── wallets.rs           # Wallet CRUD (Diesel DSL)
│   │   ├── matches.rs           # Match + extraction_history CRUD (Diesel DSL)
│   │   └── unsigned_txs.rs      # Unsigned transaction CRUD (Diesel DSL)
│   │
│   ├── scheduler/
│   │   ├── mod.rs               # spawn_schedulers() — wires all three background loops
│   │   ├── chain_indexer.rs     # Background cache refresh loop
│   │   ├── rent_extractor.rs    # Auto rent extraction loop
│   │   └── auto_matcher.rs      # Auto order matching loop
│   │
│   └── fiber/
│       ├── mod.rs               # Fiber module declarations
│       └── rpc_client.rs        # Vendored Fiber JSON-RPC client (channel queries)
│
├── migrations/
│   └── 20240623000001_initial_schema/
│       ├── up.sql               # Schema DDL (wallets, orders, matches, extraction_history,
│       │                        #   unsigned_transactions)
│       └── down.sql             # Rollback DDL
│
├── static/                      # Built web console (Vue 3 SPA, served at /admin)
│   ├── index.html
│   └── assets/                  # JS/CSS bundles (DashboardView, WalletsView, OrdersView,
│                                #   MatchesView, ChannelsView, SettingsView, etc.)
│
├── web-console/                 # Frontend source (Vue 3 + TypeScript + Vite)
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── App.vue
│       ├── main.ts
│       ├── router.ts
│       ├── views/               # DashboardView, WalletsView, OrdersView, MatchesView,
│       │                        #   ChannelsView, SettingsView
│       ├── components/          # EmptyState, StatusTag, WalletSelector, FiberAddressCell
│       └── utils/               # Formatters, API client helpers
│
└── tests/
    ├── common/mod.rs            # Shared test helpers
    ├── api_tests.rs             # API endpoint integration tests
    ├── config_tests.rs          # Config parsing + env var tests
    ├── db_tests.rs              # DB CRUD tests
    ├── hd_wallet_tests.rs       # HD wallet derivation + keystore tests
    ├── match_service_tests.rs   # Match service tests
    ├── rent_service_tests.rs    # Rent extraction + destruction tests
    ├── scheduler_tests.rs       # Scheduler logic tests
    └── wallet_service_tests.rs  # Wallet service tests
```

## Key Dependencies

| Category | Crate | Purpose |
|----------|-------|---------|
| HTTP | `actix-web` 4 | REST API framework |
| HTTP | `actix-files` | Static file serving for `/admin` |
| ORM | `diesel` 2 + `diesel_migrations` | Type-safe SQLite queries, versioned migrations |
| CKB | `opticrum-calculator` (path) | Transaction assembly pipeline |
| CKB | `opticrum-protocol` (path) | Shared types (OrderInfo, MatchInfo, etc.) |
| CKB | `ckb-cinnabar-calculator` (path) | CKB RPC client |
| Fiber | `fiber-json-types` (path) | Fiber Network JSON-RPC types |
| Crypto | `secp256k1`, `aes-gcm`, `sha2`, `hmac` | Signing + keystore encryption |
| Crypto | `bip39`, `blake2b_simd` | HD wallet (BIP39 mnemonic, BIP32 derivation) |
| CLI | `clap` 4, `toml` | Argument parsing, config file |
| Logging | `tracing`, `tracing-subscriber` | Structured async logging |
| Async | `tokio`, `async-trait` | Async runtime |
| Serialization | `serde`, `serde_json` | JSON request/response |
| HTTP client | `reqwest` (rustls-tls) | Fiber RPC proxy |
| Frontend | Vue 3, Vue Router, Chart.js, Vite | Web admin console |

## Logging

Structured logging via `tracing`. Every HTTP request logs method, path, status, and duration. State mutations (unlock, extract, destroy, match) log at `info` level. Background scheduler successes log cycle counts and elapsed times.

```bash
RUST_LOG=info cargo run      # Default: startup + state changes + errors
RUST_LOG=debug cargo run     # Also: reads, scans, cache operations, scheduler skip reasons
RUST_LOG=error cargo run     # Errors only
RUST_LOG=opticrum_server=debug cargo run  # Debug only this crate
```

Errors are logged automatically from `AppError::error_response()`, so every 4xx/5xx response produces a structured log entry.

## Database

Five tables managed by Diesel with `embed_migrations!()`:

| Table | Purpose |
|-------|---------|
| `wallets` | HD child keys (address + encrypted private key) and legacy single-key imports |
| `tracked_orders` | Orders that the server has interacted with |
| `tracked_matches` | Matches managed by the rent extractor |
| `extraction_history` | Timestamped log of every rent extraction |
| `unsigned_transactions` | Pending external signing requests (legacy flow) |

Database path is `data/opticrum.db` by default (auto-created on first run). Migrations run automatically at startup — no manual steps needed.

## Related Repositories

- [opticrum](https://github.com/ashuralyk/opticrum) — CKB contract kernel (calculator + protocol crates)
- [ckb-cinnabar](https://github.com/ashuralyk/ckb-cinnabar) — CKB RPC client library
- [Fiber Network](https://github.com/nervosnetwork/fiber) — CKB's Lightning Network (Fiber node + SDK)
