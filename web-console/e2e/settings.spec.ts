import { test, expect } from './fixtures'

test.describe('Settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/settings', { waitUntil: 'domcontentloaded' })
  })

  test('page loads with title', async ({ page }) => {
    await expect(page.locator('.page-title')).toBeVisible({ timeout: 15000 })
  })

  test('sub-tabs toggle between auto-match and signing', async ({ page }) => {
    const tabs = page.locator('.sub-tab')
    expect(await tabs.count()).toBe(2)

    await tabs.nth(0).click()
    await expect(page.locator('.config-card')).toBeVisible({ timeout: 10000 })

    await tabs.nth(1).click()
    const signingContent = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(signingContent).toBeVisible({ timeout: 10000 })
  })

  test('auto-match config loads and displays values', async ({ page }) => {
    await page.locator('.sub-tab').first().click()
    await expect(page.locator('.config-row').first()).toBeVisible({ timeout: 15000 })
  })

  test('edit → save config shows success toast with success class', async ({ page }) => {
    await page.locator('.sub-tab').first().click()
    await page.waitForSelector('.config-display', { timeout: 15000 })
    await page.locator('.card-header .btn-default').click()
    await expect(page.locator('.config-form')).toBeVisible()

    await page.locator('.config-form .btn-primary').click()
    // Must be a success toast, not error
    const toast = page.locator('.toast-item.toast-success')
    await expect(toast.first()).toBeVisible({ timeout: 5000 })
  })

  test('cancel exits edit mode without saving', async ({ page }) => {
    await page.locator('.sub-tab').first().click()
    await page.waitForSelector('.config-display', { timeout: 15000 })
    await page.locator('.card-header .btn-default').click()
    await page.locator('.config-form .btn-default').click()
    await expect(page.locator('.config-display')).toBeVisible()
  })

  test('signing tab shows content', async ({ page }) => {
    await page.locator('.sub-tab').nth(1).click()
    await page.waitForTimeout(2000)
    const content = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(content).toBeVisible({ timeout: 15000 })
  })

  test('broadcast button shows confirm modal on signed transactions', async ({ page }) => {
    await page.locator('.sub-tab').nth(1).click()
    await page.waitForTimeout(3000)
    const broadcastBtn = page.locator('.data-table .btn-primary').first()
    if (!(await broadcastBtn.isVisible().catch(() => false))) {
      test.skip(true, 'No signed transactions available')
      return
    }
    await broadcastBtn.click()
    await expect(page.locator('[data-testid="modal-overlay"]')).toBeVisible()
  })
})
