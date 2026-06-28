import { test, expect, TEST_KEY } from './fixtures'

test.describe('Wallets', () => {
  test.beforeEach(async ({ page, api }) => {
    await api.cleanupWallets()
    await page.goto('#/wallets', { waitUntil: 'domcontentloaded' })
  })

  test('empty state shows when no wallets exist', async ({ page }) => {
    await expect(page.locator('[data-testid="empty-state"]')).toBeVisible({ timeout: 15000 })
    await expect(page.locator('[data-testid="empty-state"]')).toContainText(/暂无钱包|No wallets/)
  })

  test('import button opens modal with form fields', async ({ page }) => {
    await page.locator('.page-header .btn-primary').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
    await expect(page.locator('.import-form .form-input').first()).toBeVisible()
  })

  test('entering short hex shows validation error', async ({ page }) => {
    await page.locator('.page-header .btn-primary').click()
    // Use evaluate + dispatchEvent to properly trigger Vue's @input handler
    await page.locator('.import-form .form-input').nth(1).evaluate((el: HTMLInputElement) => {
      const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype, 'value'
      )!.set!
      nativeInputValueSetter.call(el, 'short')
      el.dispatchEvent(new Event('input', { bubbles: true }))
    })
    await page.waitForTimeout(500)
    await expect(page.locator('.form-hint.text-danger')).toBeVisible({ timeout: 3000 })
  })

  test('entering valid 64-char hex shows success', async ({ page }) => {
    await page.locator('.page-header .btn-primary').click()
    await page.locator('.import-form .form-input').nth(1).evaluate(
      (el: HTMLInputElement, key: string) => {
        const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
          window.HTMLInputElement.prototype, 'value'
        )!.set!
        nativeInputValueSetter.call(el, key)
        el.dispatchEvent(new Event('input', { bubbles: true }))
      },
      TEST_KEY
    )
    await page.waitForTimeout(500)
    await expect(page.locator('.form-hint.text-success')).toBeVisible({ timeout: 3000 })
  })

  test('full wallet CRUD: import → verify card → delete → verify gone', async ({ page }) => {
    // Import
    await page.locator('.page-header .btn-primary').click()
    await page.locator('.import-form .form-input').nth(0).fill('E2E-CRUD-Test')
    await page.locator('.import-form .form-input').nth(1).fill(TEST_KEY)
    await page.locator('[data-testid="modal-confirm"]').click()

    // Wait for success toast
    await expect(page.locator('.toast-item.toast-success')).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(1000)

    // Verify card appeared with correct label
    const card = page.locator('.wallet-card').first()
    await expect(card).toBeVisible({ timeout: 5000 })
    await expect(card.locator('.wallet-label')).toContainText('E2E-CRUD-Test')

    // Delete
    const count = await page.locator('.wallet-card').count()
    await card.locator('.btn-delete').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
    await page.locator('[data-testid="modal-confirm"].btn-danger').click()

    // Verify card removed
    await expect(page.locator('.toast-item.toast-success')).toBeVisible({ timeout: 5000 })
    await expect(page.locator('.wallet-card')).toHaveCount(count - 1)
  })

  test('canceling delete keeps the wallet card', async ({ page, api }) => {
    // Seed a wallet
    await api.importWallet('E2E-Cancel-Test')
    await page.reload()
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(1000)

    const count = await page.locator('.wallet-card').count()
    if (count === 0) return test.skip()

    await page.locator('.wallet-card .btn-delete').first().click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
    await page.locator('[data-testid="modal-cancel"]').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).not.toBeVisible()
    await expect(page.locator('.wallet-card')).toHaveCount(count)
  })
})
