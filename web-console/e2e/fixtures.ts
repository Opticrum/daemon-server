import { test as base, type APIRequestContext, type Page } from '@playwright/test'

export const TEST_KEY = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'

const API = 'http://localhost:9876/api/console'

export class ApiHelpers {
  constructor(private request: APIRequestContext) {}

  async importWallet(label: string, keyHex = TEST_KEY) {
    const res = await this.request.post(`${API}/wallets`, { data: { label, private_key_hex: keyHex } })
    if (!res.ok()) return null
    return res.json() as Promise<{ id: number; label: string; ckb_address: string }>
  }

  async deleteWallet(id: number) {
    await this.request.delete(`${API}/wallets/${id}`)
  }

  async listWallets() {
    const res = await this.request.get(`${API}/wallets`)
    return res.json() as Promise<any[]>
  }

  async getDashboard() {
    const res = await this.request.get(`${API}/dashboard`)
    if (!res.ok()) return null
    return res.json() as Promise<any>
  }

  async getConfig() {
    const res = await this.request.get(`${API}/config`)
    return res.json() as Promise<any>
  }

  /** Seed a wallet via API, return its id. Teardown: delete after test. */
  async seedWallet(label = 'E2E-Test-Wallet') {
    const w = await this.importWallet(label)
    return w?.id ?? null
  }

  /** Clean up all wallets matching a label prefix. */
  async cleanupWallets(prefix = 'E2E-') {
    const wallets = await this.listWallets()
    for (const w of wallets) {
      if (w.label?.startsWith(prefix)) {
        await this.deleteWallet(w.id).catch(() => {})
      }
    }
  }
}

type Fixtures = {
  api: ApiHelpers
}

export const test = base.extend<Fixtures>({
  api: async ({ request }, use) => {
    const helpers = new ApiHelpers(request)
    await helpers.cleanupWallets()
    await use(helpers)
    await helpers.cleanupWallets()
  },
})

export { expect } from '@playwright/test'
