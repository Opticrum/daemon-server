## Learned User Preferences

- Long on-chain hashes and addresses in admin tables should display truncated on one line with a hover tooltip for the full value (not full inline, not multi-line wrap).

## Learned Workspace Facts

- Admin panel is a Vue 3 SPA in `web-console/`; built output goes to `static/` and is served at `/admin/` by the Rust backend.
- After admin UI changes, run `cd web-console && npm run build` to refresh `static/` before testing at `/admin/`.
- `static/` is gitignored; the built admin assets are not committed to the repo.
- Playwright E2E tests live in `web-console/e2e/` with baseURL `http://localhost:9876/admin`; the Playwright webServer starts only the Rust backend and does not run the frontend build.
