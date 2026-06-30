import { test, expect } from './fixtures'

test.describe('Orders', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/orders', { waitUntil: 'domcontentloaded' })
  })

  test('page loads with title', async ({ page }) => {
    await expect(page.locator('.page-title')).toBeVisible({ timeout: 15000 })
  })

  test('scan button is visible', async ({ page }) => {
    await expect(page.locator('.page-header .btn-primary')).toBeVisible({ timeout: 5000 })
  })

  test('page renders empty state or table after scan', async ({ page }) => {
    await page.waitForTimeout(8000)
    const content = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(content).toBeVisible({ timeout: 15000 })
  })

  test('match button opens modal with form fields', async ({ page }) => {
    await page.waitForTimeout(8000)
    const matchBtn = page.locator('.data-table .btn-primary').first()
    const hasOrders = await matchBtn.isVisible().catch(() => false)
    if (!hasOrders) {
      test.skip(true, 'No on-chain orders available — skipping match modal test')
      return
    }
    await matchBtn.click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
    await expect(page.locator('.match-form .form-input').first()).toBeVisible()
  })
})

test.describe('Signer wallets endpoint', () => {
  test('returns empty array when no HD wallet exists', async ({ api }) => {
    const wallets = await api.getSignerWallets()
    expect(wallets).toEqual([])
  })

  test('returns addresses after HD wallet created and unlocked', async ({ api }) => {
    // Clean up any leftover HD wallet from previous runs.
    await api.deleteHdWallet()

    // Create an HD wallet.
    const created = await api.createHdWallet('E2E-Signer-Test', 'e2e-test-pw', 3)
    expect(created).not.toBeNull()
    expect(created!.children).toHaveLength(3)

    // After creation the wallet is already unlocked; signer wallets should be populated.
    const wallets = await api.getSignerWallets()
    expect(wallets).toHaveLength(3)
    // Each entry should have a CKB address and derivation index.
    for (let i = 0; i < wallets.length; i++) {
      expect(wallets[i].ckb_address).toBeTruthy()
      expect(wallets[i].derivation_index).toBe(i)
    }

    // Clean up.
    await api.deleteHdWallet()

    // After deletion the signer should be locked again.
    const after = await api.getSignerWallets()
    expect(after).toEqual([])
  })
})

test.describe('Match modal wallet selector', () => {
  const HD_PASSWORD = 'e2e-match-test-pw'

  test.beforeEach(async ({ api }) => {
    await api.deleteHdWallet()
  })

  test.afterEach(async ({ api }) => {
    await api.deleteHdWallet()
  })

  test('shows wallet selector dropdown when HD wallet is unlocked', async ({ page, api }) => {
    // Create and unlock HD wallet.
    const created = await api.createHdWallet('E2E-Match-Test', HD_PASSWORD, 3)
    expect(created).not.toBeNull()

    await page.goto('#/orders', { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(8000)

    // If there are on-chain orders, click the first Match button.
    const matchBtn = page.locator('.data-table .btn-primary').first()
    const hasOrders = await matchBtn.isVisible().catch(() => false)
    if (!hasOrders) {
      test.skip(true, 'No on-chain orders available — skipping wallet selector UI test')
      return
    }
    await matchBtn.click()

    // Modal should be visible.
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()

    // The seller address field should be a <select> dropdown, not a text <input>.
    await expect(page.locator('.match-form select.form-select')).toBeVisible({ timeout: 5000 })
    const textInput = page.locator('.match-form input[placeholder="ckb1q..."]')
    await expect(textInput).not.toBeVisible()

    // The dropdown should have 4 options (1 placeholder + 3 wallets).
    const options = page.locator('.match-form select.form-select option')
    await expect(options).toHaveCount(4)

    // The first real option should contain a derivation index and address fragment.
    const firstWalletOption = options.nth(1)
    const optionText = await firstWalletOption.textContent()
    expect(optionText).toContain('#0')
    expect(optionText).toContain('—')

    // Close the modal.
    await page.locator('[data-testid="modal-cancel"]').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).not.toBeVisible()
  })

  test('shows fallback text input when no HD wallet', async ({ page }) => {
    // Ensure no HD wallet exists.
    await page.goto('#/orders', { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(8000)

    const matchBtn = page.locator('.data-table .btn-primary').first()
    const hasOrders = await matchBtn.isVisible().catch(() => false)
    if (!hasOrders) {
      test.skip(true, 'No on-chain orders available — skipping fallback text input test')
      return
    }
    await matchBtn.click()

    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()

    // The seller address field should be a text input, not a select.
    await expect(page.locator('.match-form input[placeholder="ckb1q..."]')).toBeVisible({ timeout: 5000 })
    const selectEl = page.locator('.match-form select.form-select')
    await expect(selectEl).not.toBeVisible()

    // The hint about unlocking the HD wallet should be visible.
    await expect(page.locator('.match-form .form-hint')).toBeVisible({ timeout: 5000 })

    // Close the modal.
    await page.locator('[data-testid="modal-cancel"]').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).not.toBeVisible()
  })
})
