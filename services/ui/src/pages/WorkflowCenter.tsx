import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { apiGet, apiPost } from '@/api/client';
import { AlertCircle, Trash2 } from 'lucide-react';
import { SubmitWorkflowDialog } from '@/components/SubmitWorkflowDialog';
import { NoopExecutorBanner } from '@/components/NoopExecutorBanner';
import { useI18n } from '@/i18n/I18nProvider';
import { useRunStateLabel } from '@/i18n/I18nProvider';
import type { WesState } from '@/api/types';

interface RunSummary {
  run_id: string;
  state?: WesState;
  workflow_type?: string;
  workflow_url?: string;
  start_time?: string;
  tags?: Record<string, string>;
}

interface RunListResponse {
  runs: RunSummary[];
  next_page_token?: string;
  orphan_queued_count?: number;
}

function workflowLabel(run: RunSummary): string {
  const tagName = run.tags?.name ?? run.tags?.workflow_name;
  if (tagName) return tagName;
  const url = run.workflow_url ?? '';
  const toolsMatch = url.match(/\/tools\/([^/]+)/);
  if (toolsMatch) return toolsMatch[1];
  try {
    const path = url.startsWith('http') ? new URL(url).pathname : url;
    const segments = path.split('/').filter(Boolean);
    const last = segments[segments.length - 1];
    if (last && !['CWL', 'WDL', 'NFL', 'SMK'].includes(last.toUpperCase())) {
      return last;
    }
    if (segments.length >= 2) return segments[segments.length - 2];
  } catch {
    /* ignore */
  }
  return run.workflow_type ?? 'Workflow';
}

function formatRunTime(iso?: string): string | null {
  if (!iso) return null;
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return null;
  }
}

const ACTIVE_STATES: WesState[] = ['QUEUED', 'INITIALIZING', 'RUNNING', 'PAUSED', 'CANCELING'];

export function WorkflowCenter() {
  const { t } = useI18n();
  const runStateLabel = useRunStateLabel();
  const queryClient = useQueryClient();

  const { data, isLoading, error } = useQuery({
    queryKey: ['wes', 'runs'],
    queryFn: () => apiGet<RunListResponse>('/ga4gh/wes/v1/runs?page_size=20'),
    retry: false,
    refetchInterval: (q) => {
      const runs = q.state.data?.runs ?? [];
      const hasActive = runs.some((r) => r.state && ACTIVE_STATES.includes(r.state));
      return hasActive ? 4000 : false;
    },
  });

  const cleanupMutation = useMutation({
    mutationFn: () =>
      apiPost<{ reconciled: number; run_ids: string[] }>(
        '/ga4gh/wes/v1/runs/stale/reconcile?older_than_secs=0',
        {},
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
  });

  const runs = data?.runs ?? [];
  const orphanCount = data?.orphan_queued_count ?? 0;

  return (
    <div className="space-y-6">
      <NoopExecutorBanner />
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('workflows.title')}</h1>
          <p className="text-muted-foreground">{t('workflows.subtitle')}</p>
        </div>
        <SubmitWorkflowDialog disabled={!!error} />
      </div>
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {t('workflows.unavailable')}
        </div>
      )}
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <CardTitle>{t('workflows.runs')}</CardTitle>
              <p className="text-sm text-muted-foreground">{t('workflows.runsHint')}</p>
            </div>
            {orphanCount > 0 && (
              <Button
                variant="outline"
                size="sm"
                disabled={cleanupMutation.isPending}
                onClick={() => cleanupMutation.mutate()}
              >
                <Trash2 className="h-4 w-4 mr-2" />
                {t('workflows.cleanupStale', { count: orphanCount })}
              </Button>
            )}
          </div>
          {orphanCount > 0 && (
            <p className="text-sm text-amber-600 dark:text-amber-400">{t('workflows.cleanupStaleHint')}</p>
          )}
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">{t('common.loading')}</p>}
          {!isLoading && runs.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">{t('workflows.noRuns')}</p>
          )}
          {!isLoading && runs.length > 0 && (
            <ul className="space-y-3">
              {runs.map((r) => {
                const label = workflowLabel(r);
                const started = formatRunTime(r.start_time);
                return (
                  <li key={r.run_id}>
                    <Link
                      to={`/workflows/runs/${r.run_id}` as any}
                      className="block rounded-md border p-3 hover:bg-muted/50 transition-colors"
                    >
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium">{label}</span>
                        {r.workflow_type && (
                          <Badge variant="outline" className="text-xs uppercase">
                            {r.workflow_type}
                          </Badge>
                        )}
                        {r.state && (
                          <Badge
                            variant={
                              r.state === 'COMPLETE'
                                ? 'default'
                                : r.state === 'EXECUTOR_ERROR' || r.state === 'SYSTEM_ERROR'
                                  ? 'destructive'
                                  : 'secondary'
                            }
                            className="text-xs"
                          >
                            {runStateLabel(r.state)}
                          </Badge>
                        )}
                      </div>
                      <p className="text-muted-foreground mt-1 font-mono text-xs">{r.run_id}</p>
                      {started && (
                        <p className="text-muted-foreground mt-1 text-xs">
                          {t('workflows.runStarted', { time: started })}
                        </p>
                      )}
                    </Link>
                  </li>
                );
              })}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
