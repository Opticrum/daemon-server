import { test, expect } from './fixtures'

test.describe('Channels', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/channels', { waitUntil: 'domcontentloaded' })
  })

  test('page loads with title', async ({ page }) => {
    await expect(page.locator('.page-title')).toBeVisible({ timeout: 15000 })
  })

  test('search input and scan button visible', async ({ page }) => {
    await expect(page.locator('.search-input')).toBeVisible()
    await expect(page.locator('.search-bar .btn-primary')).toBeVisible()
  })

  test('pre-scan empty state shown with prompt text', async ({ page }) => {
    await expect(page.locator('[data-testid="empty-state"]')).toBeVisible({ timeout: 10000 })
    await expect(page.locator('[data-testid="empty-state"]')).toContainText(/Lock Hash|锁哈希/)
  })

  test('entering hash and clicking scan triggers search', async ({ page }) => {
    await page.locator('.search-input').fill('a'.repeat(64))
    await page.locator('.search-bar .btn-primary').click()
    await page.waitForTimeout(3000)
    const content = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(content).toBeVisible()
  })

  test('enter key submits search', async ({ page }) => {
    await page.locator('.search-input').fill('a'.repeat(64))
    await page.locator('.search-input').press('Enter')
    await page.waitForTimeout(3000)
    const content = page.locator('[data-testid="empty-state"], .data-table').first()
    await expect(content).toBeVisible()
  })
})
