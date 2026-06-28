import { test, expect } from './fixtures'

test.describe('i18n', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/dashboard', { waitUntil: 'domcontentloaded' })
  })

  test('default locale is zh-CN', async ({ page }) => {
    const toggle = page.locator('[data-testid="lang-toggle"]')
    await expect(toggle).toBeVisible({ timeout: 15000 })
    await expect(toggle).toHaveText('中文')
  })

  test('lang toggle switches to EN and changes sidebar labels', async ({ page }) => {
    await page.locator('[data-testid="lang-toggle"]').click()
    await expect(page.locator('[data-testid="lang-toggle"]')).toHaveText('EN')
    await expect(page.locator('[data-testid="nav-dashboard"] .nav-label')).toContainText('Overview')
    await expect(page.locator('[data-testid="nav-wallets"] .nav-label')).toContainText('Wallets')
  })

  test('lang toggle switches back to zh-CN', async ({ page }) => {
    const toggle = page.locator('[data-testid="lang-toggle"]')
    await toggle.click()
    await toggle.click()
    await expect(toggle).toHaveText('中文')
    await expect(page.locator('[data-testid="nav-dashboard"] .nav-label')).toContainText('运营概览')
  })

  test('brand name changes with locale', async ({ page }) => {
    await expect(page.locator('[data-testid="brand-name"]')).toContainText('Opticrum 管理控制台')
    await page.locator('[data-testid="lang-toggle"]').click()
    await expect(page.locator('[data-testid="brand-name"]')).toContainText('Opticrum')
  })

  test('locale persists across page reload', async ({ page }) => {
    await page.locator('[data-testid="lang-toggle"]').click()
    await expect(page.locator('[data-testid="lang-toggle"]')).toHaveText('EN')
    await page.reload()
    await page.waitForLoadState('domcontentloaded')
    await expect(page.locator('[data-testid="lang-toggle"]')).toHaveText('EN')
  })

  test('page titles change with locale', async ({ page }) => {
    // Navigate to a view with zh-CN, verify, then switch and verify
    await page.locator('[data-testid="nav-wallets"]').click()
    await expect(page.locator('.page-title')).toContainText('钱包管理')

    await page.locator('[data-testid="lang-toggle"]').click()
    await expect(page.locator('.page-title')).toContainText('Wallet')
  })
})
