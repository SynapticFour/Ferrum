import { LOCALE_LABELS, LOCALES, type Locale } from '@/i18n/locales';
import { useLocaleStore } from '@/stores/locale';
import { useI18n } from '@/i18n/I18nProvider';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Languages } from 'lucide-react';

export function LanguageSwitcher({ className }: { className?: string }) {
  const locale = useLocaleStore((s) => s.locale);
  const setLocale = useLocaleStore((s) => s.setLocale);
  const { t } = useI18n();

  return (
    <div className={className}>
      <Select value={locale} onValueChange={(v) => setLocale(v as Locale)}>
        <SelectTrigger
          className="h-8 w-full gap-2 border-0 bg-transparent px-2 text-xs shadow-none focus:ring-0"
          aria-label={t('common.language')}
        >
          <Languages className="h-3.5 w-3.5 shrink-0 opacity-70" />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {LOCALES.map((loc) => (
            <SelectItem key={loc} value={loc}>
              {LOCALE_LABELS[loc]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
