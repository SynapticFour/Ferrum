import { test, expect } from '@playwright/test';

test.describe('Arabic RTL', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('ferrum.locale', 'ar');
    });
    await page.goto('/');
  });

  test('sets document direction to rtl', async ({ page }) => {
    const dir = await page.locator('html').getAttribute('dir');
    expect(dir).toBe('rtl');
    const lang = await page.locator('html').getAttribute('lang');
    expect(lang).toBe('ar');
  });

  test('sidebar anchors on the right in rtl', async ({ page }) => {
    const aside = page.locator('aside').first();
    await expect(aside).toBeVisible();
    const box = await aside.boundingBox();
    expect(box).not.toBeNull();
    const viewport = page.viewportSize();
    expect(viewport).not.toBeNull();
    // Sidebar should occupy the right edge (x near viewport width).
    expect(box!.x + box!.width).toBeGreaterThan(viewport!.width * 0.85);
  });
});
