## Learned User Preferences

- Long on-chain hashes and addresses in admin tables should display truncated on one line with a hover tooltip for the full value (not full inline, not multi-line wrap).

## Learned Workspace Facts

- Admin panel is a Vue 3 SPA in `web-console/`; built output goes to `static/` and is served at `/admin/` by the Rust backend.
- After admin UI changes, run `cd web-console && npm run build` to refresh `static/` before testing at `/admin/`.
- `static/` is gitignored; the built admin assets are not committed to the repo.
- Playwright E2E tests live in `web-console/e2e/` with baseURL `http://localhost:9876/admin`; the Playwright webServer starts only the Rust backend and does not run the frontend build.
- Admin panel title is "Opticrum Admin Console" / "Opticrum 管理控制台" (i18n in `web-console/src/locales/`).
- Transaction signing uses the built-in HD wallet only (`HdWalletSigner`); external signing and unsigned-transaction queue UI/routes were removed.
- HD wallet signing keys load into memory when the user unlocks the keystore in Wallet Management; auto-match skips when the wallet is locked.
- Wallet unlock persists for 1 hour via an HttpOnly cookie (`opticrum_wallet_session`); the password stays in server RAM only and sessions are lost on server restart.
- HD wallet CKB addresses and lock hashes must match ckb-cli/ckb-sdk (BIP32 path `m/44'/309'/0'/0/0` with normal derivation for the last two segments; CKB2021 bech32m addresses).
- On-chain CKB balances are queried via the CKB indexer using lock args decoded from each wallet's `ckb_address`.
