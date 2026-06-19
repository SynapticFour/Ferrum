import { useI18n } from '@/i18n/I18nProvider';

export function PageLoader() {
  const { t } = useI18n();
  return (
    <div className="flex min-h-[40vh] items-center justify-center text-sm text-muted-foreground">
      {t('common.loading')}
    </div>
  );
}
