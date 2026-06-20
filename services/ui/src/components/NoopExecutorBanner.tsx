import { AlertTriangle } from 'lucide-react';
import { useAdminConfig, isNoopExecutor } from '@/hooks/useAdminConfig';
import { useI18n } from '@/i18n/I18nProvider';
import { cn } from '@/lib/utils';

export function NoopExecutorBanner({ compact }: { compact?: boolean }) {
  const { data: config } = useAdminConfig();
  const { t } = useI18n();

  if (!isNoopExecutor(config)) return null;

  if (compact) {
    return (
      <div
        role="status"
        className="flex gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-300"
      >
        <AlertTriangle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
        <span>{t('noop.compact')}</span>
      </div>
    );
  }

  return (
    <div
      role="status"
      className={cn(
        'flex gap-3 rounded-lg border border-amber-500/50 bg-amber-500/10 px-4 py-3 text-sm text-amber-800 dark:text-amber-300',
      )}
    >
      <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
      <div>
        <p className="font-medium">{t('noop.title')}</p>
        <p className="mt-1 text-amber-700/90 dark:text-amber-200/80">{t('noop.body')}</p>
        <p className="mt-1 text-xs text-amber-600/80 dark:text-amber-300/70">{t('noop.hint')}</p>
      </div>
    </div>
  );
}
