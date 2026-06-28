import { test, expect, TEST_KEY } from './fixtures'

test.describe('Matches', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/matches', { waitUntil: 'domcontentloaded' })
  })

  test('page loads with 4 filter tabs', async ({ page }) => {
    await expect(page.locator('.filter-tab').first()).toBeVisible({ timeout: 15000 })
    expect(await page.locator('.filter-tab').count()).toBe(4)
  })

  test('filter tabs switch active state', async ({ page }) => {
    const tabs = page.locator('.filter-tab')
    for (let i = 0; i < 4; i++) {
      await tabs.nth(i).click()
      await expect(tabs.nth(i)).toHaveClass(/active/)
    }
  })

  test('empty state or table renders', async ({ page }) => {
    await page.waitForTimeout(5000)
    const content = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(content).toBeVisible({ timeout: 15000 })
  })

  test('refresh button reloads data', async ({ page }) => {
    const refreshBtn = page.locator('.page-header .btn-default')
    await expect(refreshBtn).toBeVisible({ timeout: 10000 })
    await refreshBtn.click()
    await page.waitForTimeout(2000)
  })

  test('destroy button shows confirm modal on exhausted matches', async ({ page }) => {
    await page.locator('.filter-tab').nth(2).click() // Exhausted tab
    await page.waitForTimeout(2000)
    const destroyBtn = page.locator('.data-table .btn-danger').first()
    if (!(await destroyBtn.isVisible().catch(() => false))) {
      test.skip(true, 'No exhausted matches available')
      return
    }
    await destroyBtn.click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
    await expect(page.locator('[data-testid="modal-confirm"].btn-danger')).toBeVisible()
    // Cancel the modal
    await page.locator('[data-testid="modal-cancel"]').click()
    await expect(page.locator('[data-testid="modal-overlay"]')).not.toBeVisible()
  })
})
