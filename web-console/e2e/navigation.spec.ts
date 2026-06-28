import { test, expect } from './fixtures'

test.describe('Navigation', () => {
  test('sidebar loads with nav items', async ({ page }) => {
    await page.goto('#/dashboard', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('[data-testid="nav-dashboard"]')).toBeVisible({ timeout: 15000 })
  })

  test('sidebar navigates changes URL and page content', async ({ page }) => {
    await page.goto('#/dashboard', { waitUntil: 'domcontentloaded' })
    const routes = [
      { hash: 'wallets', title: /钱包管理|Wallet/ },
      { hash: 'orders', title: /链上订单|On-Chain/ },
      { hash: 'matches', title: /匹配记录|Match Records/ },
      { hash: 'channels', title: /Fiber 通道|Fiber Channels/ },
      { hash: 'settings', title: /系统设置|Settings/ },
    ]
    for (const { hash, title } of routes) {
      await page.locator(`[data-testid="nav-${hash}"]`).click()
      await expect(page).toHaveURL(new RegExp(`#/${hash}$`))
      await expect(page.locator('.page-title')).toHaveText(title, { timeout: 10000 })
    }
  })

  test('hash URL direct navigation works', async ({ page }) => {
    await page.goto('#/wallets', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('.page-title')).toBeVisible({ timeout: 15000 })
  })

  test('sidebar collapse toggle works', async ({ page }) => {
    await page.goto('#/dashboard', { waitUntil: 'domcontentloaded' })
    const sidebar = page.locator('.app-sidebar')
    await expect(sidebar).toBeVisible()
    await page.locator('[data-testid="sidebar-collapse-btn"]').click()
    await expect(sidebar).toHaveClass(/collapsed/)
  })
})
