## Learned User Preferences

- Long on-chain hashes and addresses in admin tables should display truncated on one line with a hover tooltip for the full value (not full inline, not multi-line wrap).
- Background chain cache refresh must not interrupt page navigation or force-refresh the current view; pages load data on mount/revisit from the latest cache snapshot.
- Automation Console should log all key automation operations with enough detail to follow progress without noisy or annoying message density.

## Learned Workspace Facts

- Admin panel is a Vue 3 SPA in `web-console/`; run `cd web-console && npm run build` to refresh gitignored `static/` served at `/admin/`.
- Playwright E2E tests live in `web-console/e2e/` with baseURL `http://localhost:9876/admin`; the Playwright webServer starts only the Rust backend and does not run the frontend build.
- Admin panel title is "Opticrum Admin Console" / "Opticrum 管理控制台" (i18n in `web-console/src/locales/`).
- Transaction signing uses the built-in HD wallet only (`HdWalletSigner`); external signing and unsigned-transaction queue UI/routes were removed.
- Enabling automation requires selecting an HD wallet signing address before entering the unlock password (`AutomationUnlockForm` in Settings).
- HD wallet unlock uses an HttpOnly cookie (`opticrum_wallet_session`); signing keys load into memory and auto-match skips when locked. Unlock persists until the user disables automation, resets config, or refreshes the page—admin copy should not describe timed auto-lock.
- System Settings includes a foldable Automation Console (`AutomationConsole.vue`) that polls `/api/console/scheduler/status` when expanded to monitor auto-match, rent-extraction, and chain-indexer cycles.
- HD wallet CKB addresses and lock hashes must match ckb-cli/ckb-sdk (BIP32 path `m/44'/309'/0'/0/0` with normal derivation for the last two segments; CKB2021 bech32m addresses).
- On-chain CKB balances are queried via the CKB indexer using lock args decoded from each wallet's `ckb_address`.
- Background `chain_indexer` keeps an in-memory Opticrum chain cache (orders, matches, Fiber channels, extraction history); `CachedChainProvider` wraps `ChainProvider` and serves scan reads from cache when enabled, falling back to live RPC otherwise.
- Chain cache age and manual refresh live in the AppHeader status bar via `useChainCache` composable and `/api/console/chain-cache/*` endpoints.
