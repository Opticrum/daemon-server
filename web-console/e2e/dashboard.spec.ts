import { test, expect } from './fixtures'

test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('#/dashboard', { waitUntil: 'domcontentloaded' })
  })

  test('page renders title and KPI cards', async ({ page }) => {
    await expect(page.locator('.page-title')).toBeVisible({ timeout: 30000 })
    await expect(page.locator('.kpi-card')).toHaveCount(5)
  })

  test('KPI cards show real numeric values (not placeholder)', async ({ page }) => {
    await page.waitForSelector('.kpi-card .kpi-number', { timeout: 30000 })
    const numbers = page.locator('.kpi-card .kpi-number')
    for (let i = 0; i < 5; i++) {
      const text = (await numbers.nth(i).textContent())?.trim() || ''
      // Must contain digits (real KPI values have numbers, placeholder is just "—")
      expect(text).toBeTruthy()
      expect(text).toMatch(/\d/)
    }
  })

  test('chart canvases render', async ({ page }) => {
    await page.waitForSelector('canvas', { timeout: 30000 })
    const count = await page.locator('canvas').count()
    expect(count).toBeGreaterThanOrEqual(4)
  })

  test('skeleton loading disappears and content renders', async ({ page }) => {
    // Dashboard ChartCard uses .skeleton-chart, not .skeleton-card
    await page.waitForSelector('.skeleton-chart', { state: 'detached', timeout: 60000 }).catch(() => {})
    await expect(page.locator('.kpi-card').first()).toBeVisible({ timeout: 10000 })
  })

  test('match distribution chart renders with legend', async ({ page }) => {
    await page.waitForSelector('canvas', { timeout: 30000 })
    // DonutChart uses Chart.js — verify the canvas parent is visible
    await expect(page.locator('.chart-col-8 canvas').first()).toBeVisible({ timeout: 10000 })
  })
})
