import { Link, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiGet, apiPost } from '@/api/client';
import { RunLineageTab } from '@/components/RunLineageTab';
import { RunMetricsTab } from '@/components/RunMetricsTab';
import { Button } from '@/components/ui/button';
import { ArrowLeft, RotateCw } from 'lucide-react';
import { WorkflowStateBadge } from '@/components/WorkflowStateBadge';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';

interface RunLog {
  run_id: string;
  state: string;
  resumed_from_run_id?: string | null;
  request?: { workflow_type?: string; workflow_url?: string };
  run_log?: { stdout?: string; stderr?: string };
}

export function RunDetailPage() {
  const params = useParams({ strict: false }) as { runId?: string };
  const id = params.runId ?? '';
  const queryClient = useQueryClient();

  const { data: run, isLoading, error } = useQuery({
    queryKey: ['wes', 'run', id],
    queryFn: () => apiGet<RunLog>(`/ga4gh/wes/v1/runs/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  const resumeMutation = useMutation({
    mutationFn: (body?: { override_params?: Record<string, unknown> }) =>
      apiPost<{ run_id: string; resumed_from: string; cached_tasks: number; tasks_to_rerun: number; estimated_time_saved: string }>(
        `/ga4gh/wes/v1/runs/${encodeURIComponent(id)}/resume`,
        body ?? {}
      ),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['wes', 'runs'] });
      const base = typeof window !== 'undefined' && window.location.pathname.startsWith('/ui') ? '/ui' : '';
      window.location.href = `${base}/workflows/runs/${data.run_id}`;
    },
  });

  if (!id) return <p className="text-muted-foreground">No run ID.</p>;
  if (isLoading) return <p className="text-muted-foreground">Loading…</p>;
  if (error || !run) return <p className="text-destructive">Run not found.</p>;

  const terminalStates = ['COMPLETE', 'EXECUTOR_ERROR', 'SYSTEM_ERROR', 'CANCELED'];
  const canResume = terminalStates.includes(run.state);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2 flex-wrap">
        <Button variant="ghost" size="icon" asChild>
          <Link to={"/workflows" as any}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <h1 className="text-2xl font-bold">Run {run.run_id}</h1>
        <WorkflowStateBadge state={run.state as 'RUNNING' | 'COMPLETE' | 'QUEUED'} />
        {run.resumed_from_run_id && (
          <span className="text-sm text-muted-foreground">
            Resumed from{' '}
            <Link to={"/workflows/runs/" + run.resumed_from_run_id} className="text-primary underline">
              {run.resumed_from_run_id}
            </Link>
          </span>
        )}
        {canResume && (
          <Dialog>
            <DialogTrigger asChild>
              <Button variant="outline" size="sm">
                <RotateCw className="mr-2 h-4 w-4" />
                {run.state === 'COMPLETE' ? 'Re-run' : 'Resume from checkpoint'}
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>{run.state === 'COMPLETE' ? 'Re-run workflow' : 'Resume from checkpoint'}</DialogTitle>
                <DialogDescription>
                  This will create a new run reusing cached outputs where possible. You can override workflow parameters.
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button
                  onClick={() => resumeMutation.mutate(undefined)}
                  disabled={resumeMutation.isPending}
                >
                  {resumeMutation.isPending ? 'Creating…' : 'Confirm'}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        )}
      </div>
      <Tabs defaultValue="log">
        <TabsList>
          <TabsTrigger value="log">Log</TabsTrigger>
          <TabsTrigger value="metrics">Metrics</TabsTrigger>
          <TabsTrigger value="lineage">Lineage</TabsTrigger>
        </TabsList>
        <TabsContent value="log">
          <Card>
            <CardHeader><CardTitle>Run log</CardTitle></CardHeader>
            <CardContent className="text-sm">
              {run.request?.workflow_url && (
                <p className="text-muted-foreground">Workflow: {run.request.workflow_url}</p>
              )}
              {run.run_log?.stdout && (
                <pre className="mt-2 overflow-auto max-h-96 rounded bg-muted p-2 text-xs">{run.run_log.stdout}</pre>
              )}
              {!run.run_log?.stdout && <p className="text-muted-foreground">No stdout yet.</p>}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="metrics">
          <RunMetricsTab runId={id} />
        </TabsContent>
        <TabsContent value="lineage">
          <RunLineageTab runId={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
