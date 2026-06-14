export const LOCALES = ['en', 'de', 'fr', 'ar'] as const;
export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = 'en';
export const LOCALE_STORAGE_KEY = 'ferrum.locale';
export const RTL_LOCALES = new Set<Locale>(['ar']);

export const LOCALE_LABELS: Record<Locale, string> = {
  en: 'English',
  de: 'Deutsch',
  fr: 'Français',
  ar: 'العربية',
};

export function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

/** Prefer stored preference, then browser languages, then English. */
export function detectSystemLocale(): Locale {
  if (typeof navigator === 'undefined') return DEFAULT_LOCALE;
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const lang of langs) {
    const code = lang.split('-')[0]?.toLowerCase();
    if (code && isLocale(code)) return code;
  }
  return DEFAULT_LOCALE;
}
