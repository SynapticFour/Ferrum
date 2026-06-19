import { test, expect } from '@playwright/test';

test.describe('Ferrum UI smoke', () => {
  test('dashboard loads with navigation', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { name: /welcome|willkommen|bienvenue|مرحباً/i })).toBeVisible();
    await expect(page.locator('aside').getByRole('link', { name: /^data$|^daten$|^données$|^البيانات$/i })).toBeVisible();
  });

  test('study setup wizard renders step rail', async ({ page }) => {
    await page.goto('/study/setup');
    await expect(page.getByTestId('study-setup-wizard')).toBeVisible();
    await expect(page.getByRole('button', { name: /next|weiter|suivant|التالي/i })).toBeVisible();
  });

  test('data browser catalog tab and import dialog', async ({ page }) => {
    await page.goto('/data');
    await expect(page.getByRole('heading', { name: /^data$|^daten$|^données$|^البيانات$/i })).toBeVisible();
    await page.getByTestId('data-catalog-tab').click();
    await expect(page.getByText(/catalog|katalog|catalogue|كتالوج/i).first()).toBeVisible();
    await page.getByTestId('import-to-drs-trigger').click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByText(/upload|hochladen|téléverser|رفع/i).first()).toBeVisible();
  });

  test('register tool wizard opens from tools page', async ({ page }) => {
    await page.goto('/tools');
    await page.getByTestId('register-tool-trigger').click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByText(/preset|vorlage|modèle|قالب/i).first()).toBeVisible();
  });
});
