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
