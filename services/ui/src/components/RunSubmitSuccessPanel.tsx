import { Link } from '@tanstack/react-router';
import { CheckCircle2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useI18n } from '@/i18n/I18nProvider';

export interface RunSubmitSuccessPanelProps {
  runIds: string[];
  cohortCount?: number;
  onDismiss?: () => void;
  className?: string;
}

export function RunSubmitSuccessPanel({
  runIds,
  cohortCount,
  onDismiss,
  className,
}: RunSubmitSuccessPanelProps) {
  const { t } = useI18n();
  const count = cohortCount ?? runIds.length;

  return (
    <div
      className={
        className ??
        'rounded-md border border-emerald-500/40 bg-emerald-500/10 px-3 py-3 text-sm text-emerald-800 dark:text-emerald-300'
      }
    >
      <p className="flex items-center gap-2 font-medium">
        <CheckCircle2 className="h-4 w-4 shrink-0" />
        {count > 1
          ? t('workflows.runSubmitSuccessCohort', { count: String(count) })
          : t('workflows.runSubmitSuccess', { id: runIds[0]?.slice(0, 12) ?? '—' })}
      </p>
      <div className="mt-2 flex flex-wrap gap-2">
        {runIds.slice(0, 3).map((runId) => (
          <Button key={runId} variant="outline" size="sm" className="h-8 text-xs" asChild>
            <Link to={`/workflows/runs/${runId}` as any}>{t('workflows.viewRun', { id: runId.slice(0, 8) })}</Link>
          </Button>
        ))}
        <Button variant="default" size="sm" className="h-8 text-xs" asChild>
          <Link to={'/workflows' as any}>{t('workflows.viewAllRuns')}</Link>
        </Button>
        {onDismiss && (
          <Button variant="ghost" size="sm" className="h-8 text-xs" onClick={onDismiss}>
            {t('common.dismiss')}
          </Button>
        )}
      </div>
    </div>
  );
}
