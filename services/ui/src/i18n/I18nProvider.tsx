import { createContext, useContext, useEffect, useMemo, type ReactNode } from 'react';
import { de } from './de';
import { en, type Messages } from './en';
import { fr } from './fr';
import { ar } from './ar';
import { DEFAULT_LOCALE, RTL_LOCALES, type Locale } from './locales';
import { useLocaleStore } from '@/stores/locale';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type TranslationNode = string | { [key: string]: any };

const bundles: Record<Locale, Messages> = { en, de, fr, ar };

function lookupInBundle(bundle: Messages, key: string): string | undefined {
  const keys = key.split('.');
  let node: TranslationNode | undefined = bundle;
  for (const k of keys) {
    if (!node || typeof node === 'string') return undefined;
    node = (node as Record<string, TranslationNode>)[k];
  }
  return typeof node === 'string' ? node : undefined;
}

function lookup(locale: Locale, key: string): string {
  const localized = lookupInBundle(bundles[locale] ?? bundles[DEFAULT_LOCALE], key);
  if (localized !== undefined) return localized;
  if (locale !== DEFAULT_LOCALE) {
    const fallback = lookupInBundle(bundles[DEFAULT_LOCALE], key);
    if (fallback !== undefined) return fallback;
  }
  return key;
}

export type TranslateFn = (key: string, vars?: Record<string, string | number>) => string;

interface I18nContextValue {
  locale: Locale;
  t: TranslateFn;
  dir: 'ltr' | 'rtl';
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const locale = useLocaleStore((s) => s.locale);
  const initLocale = useLocaleStore((s) => s.initLocale);

  useEffect(() => {
    initLocale();
  }, [initLocale]);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = RTL_LOCALES.has(locale) ? 'rtl' : 'ltr';
  }, [locale]);

  const value = useMemo<I18nContextValue>(() => {
    const t: TranslateFn = (key, vars) => {
      let result = lookup(locale, key);
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          result = result.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v));
        }
      }
      return result;
    };
    return { locale, t, dir: RTL_LOCALES.has(locale) ? 'rtl' : 'ltr' };
  }, [locale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error('useI18n must be used within I18nProvider');
  return ctx;
}

/** Run-state labels with fallback to raw state string. */
export function useRunStateLabel(): (state: string) => string {
  const { t } = useI18n();
  return (state: string) => {
    const key = `runStates.${state}`;
    const label = t(key);
    return label === key ? state : label;
  };
}
