import { lazy, Suspense } from 'react';
import { Link, useNavigate, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiGet, apiGetText, apiPost } from '@/api/client';
import { RunLineageTab } from '@/components/RunLineageTab';
import { RunResultsTab } from '@/components/RunResultsTab';
import { LiveLogViewer } from '@/components/LiveLogViewer';
import { Button } from '@/components/ui/button';
import { ArrowLeft, RotateCw } from 'lucide-react';
import { WorkflowStateBadge } from '@/components/WorkflowStateBadge';
import { NoopExecutorBanner } from '@/components/NoopExecutorBanner';
import { useLiveRunLogs } from '@/hooks/useIngestJobs';
import { useI18n } from '@/i18n/I18nProvider';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';

const RunMetricsTab = lazy(() =>
  import('@/components/RunMetricsTab').then((m) => ({ default: m.RunMetricsTab })),
);

interface RunLog {
  run_id: string;
  state: string;
  resumed_from_run_id?: string | null;
  request?: { workflow_type?: string; workflow_url?: string };
  run_log?: { stdout?: string; stderr?: string };
}

export function RunDetailPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const params = useParams({ strict: false }) as { runId?: string };
  const id = params.runId ?? '';
  const queryClient = useQueryClient();

  const { data: run, isLoading, error } = useQuery({
    queryKey: ['wes', 'run', id],
    queryFn: () => apiGet<RunLog>(`/ga4gh/wes/v1/runs/${encodeURIComponent(id)}`),
    enabled: !!id,
    refetchInterval: (q) => {
      const state = q.state.data?.state;
      if (state === 'RUNNING' || state === 'QUEUED' || state === 'INITIALIZING') return 3000;
      return false;
    },
  });

  const isActive =
    run?.state === 'RUNNING' || run?.state === 'QUEUED' || run?.state === 'INITIALIZING';
  const { lines: liveLines, unavailable: liveUnavailable } = useLiveRunLogs(id, !!isActive);

  const { data: storedStdout } = useQuery({
    queryKey: ['wes', 'run', id, 'stdout'],
    queryFn: () => apiGetText(`/ga4gh/wes/v1/runs/${encodeURIComponent(id)}/logs/stdout`),
    enabled: !!id && !isActive,
    retry: false,
  });

  const { data: storedStderr } = useQuery({
    queryKey: ['wes', 'run', id, 'stderr'],
    queryFn: () => apiGetText(`/ga4gh/wes/v1/runs/${encodeURIComponent(id)}/logs/stderr`),
    enabled: !!id && !isActive,
    retry: false,
  });

  const resumeMutation = useMutation({
    mutationFn: (body?: { override_params?: Record<string, unknown> }) =>
      apiPost<{ run_id: string; resumed_from: string; cached_tasks: number; tasks_to_rerun: number; estimated_time_saved: string }>(
        `/ga4gh/wes/v1/runs/${encodeURIComponent(id)}/resume`,
        body ?? {},
      ),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['wes', 'runs'] });
      void (navigate as (opts: { to: string }) => void)({ to: `/workflows/runs/${data.run_id}` });
    },
  });

  if (!id) return <p className="text-muted-foreground">{t('run.noId')}</p>;
  if (isLoading) return <p className="text-muted-foreground">{t('common.loading')}</p>;
  if (error || !run) return <p className="text-destructive">{t('run.notFound')}</p>;

  const terminalStates = ['COMPLETE', 'EXECUTOR_ERROR', 'SYSTEM_ERROR', 'CANCELED'];
  const canResume = terminalStates.includes(run.state);

  const staticLines = [
    ...(storedStdout?.split('\n').filter((l) => l.length > 0) ?? []),
    ...(storedStderr?.split('\n').filter((l) => l.length > 0) ?? []),
  ];
  const displayLines = liveLines.length > 0 ? liveLines : staticLines;
  const defaultTab = run.state === 'COMPLETE' ? 'results' : 'log';

  return (
    <div className="space-y-6">
      <NoopExecutorBanner />
      <div className="flex items-center gap-2 flex-wrap">
        <Button variant="ghost" size="icon" asChild>
          <Link to={'/workflows' as any}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <h1 className="text-2xl font-bold">Run {run.run_id}</h1>
        <WorkflowStateBadge state={run.state as 'RUNNING' | 'COMPLETE' | 'QUEUED'} />
        {run.resumed_from_run_id && (
          <span className="text-sm text-muted-foreground">
            {t('run.resumedFrom')}{' '}
            <Link to={'/workflows/runs/' + run.resumed_from_run_id} className="text-primary underline">
              {run.resumed_from_run_id}
            </Link>
          </span>
        )}
        {canResume && (
          <Dialog>
            <DialogTrigger asChild>
              <Button variant="outline" size="sm">
                <RotateCw className="mr-2 h-4 w-4" />
                {run.state === 'COMPLETE' ? t('run.rerun') : t('run.resume')}
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>
                  {run.state === 'COMPLETE' ? t('run.rerunTitle') : t('run.resumeTitle')}
                </DialogTitle>
                <DialogDescription>{t('run.rerunDesc')}</DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button onClick={() => resumeMutation.mutate(undefined)} disabled={resumeMutation.isPending}>
                  {resumeMutation.isPending ? t('run.creating') : t('common.confirm')}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        )}
      </div>
      <Tabs defaultValue={defaultTab}>
        <TabsList>
          <TabsTrigger value="log">{isActive ? t('run.liveLog') : t('run.log')}</TabsTrigger>
          <TabsTrigger value="results">{t('run.results')}</TabsTrigger>
          <TabsTrigger value="metrics">{t('run.metrics')}</TabsTrigger>
          <TabsTrigger value="lineage">{t('run.lineage')}</TabsTrigger>
        </TabsList>
        <TabsContent value="log">
          <Card>
            <CardHeader>
              <CardTitle>{isActive ? t('run.liveLog') : t('run.runLog')}</CardTitle>
              {run.request?.workflow_url && (
                <p className="text-sm text-muted-foreground truncate">{run.request.workflow_url}</p>
              )}
              {isActive && liveUnavailable && liveLines.length === 0 && (
                <p className="text-xs text-amber-600 dark:text-amber-400">{t('run.streamUnavailable')}</p>
              )}
            </CardHeader>
            <CardContent>
              <LiveLogViewer lines={displayLines} maxHeight="24rem" />
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="results">
          <RunResultsTab runId={id} />
        </TabsContent>
        <TabsContent value="metrics">
          <Suspense fallback={<p className="text-muted-foreground">{t('common.loading')}</p>}>
            <RunMetricsTab runId={id} />
          </Suspense>
        </TabsContent>
        <TabsContent value="lineage">
          <RunLineageTab runId={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
