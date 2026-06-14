import { create } from 'zustand';
import {
  DEFAULT_LOCALE,
  LOCALE_STORAGE_KEY,
  detectSystemLocale,
  isLocale,
  type Locale,
} from '@/i18n/locales';

interface LocaleStore {
  locale: Locale;
  initialized: boolean;
  initLocale: () => void;
  setLocale: (locale: Locale) => void;
}

function readStoredLocale(): Locale | null {
  try {
    const raw = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (raw && isLocale(raw)) return raw;
  } catch {
    /* private browsing */
  }
  return null;
}

export const useLocaleStore = create<LocaleStore>((set) => ({
  locale: DEFAULT_LOCALE,
  initialized: false,
  initLocale: () => {
    const stored = readStoredLocale();
    const locale = stored ?? detectSystemLocale();
    set({ locale, initialized: true });
  },
  setLocale: (locale) => {
    try {
      localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch {
      /* private browsing */
    }
    set({ locale });
  },
}));
