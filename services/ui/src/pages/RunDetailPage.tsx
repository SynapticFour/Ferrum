import { Link, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { RunLineageTab } from '@/components/RunLineageTab';
import { Button } from '@/components/ui/button';
import { ArrowLeft } from 'lucide-react';
import { WorkflowStateBadge } from '@/components/WorkflowStateBadge';

interface RunLog {
  run_id: string;
  state: string;
  request?: { workflow_type?: string; workflow_url?: string };
  run_log?: { stdout?: string; stderr?: string };
}

export function RunDetailPage() {
  const { runId } = useParams({ strict: false });
  const id = runId ?? '';

  const { data: run, isLoading, error } = useQuery({
    queryKey: ['wes', 'run', id],
    queryFn: () => apiGet<RunLog>(`/ga4gh/wes/v1/runs/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  if (!id) return <p className="text-muted-foreground">No run ID.</p>;
  if (isLoading) return <p className="text-muted-foreground">Loading…</p>;
  if (error || !run) return <p className="text-destructive">Run not found.</p>;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to="/workflows"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <h1 className="text-2xl font-bold">Run {run.run_id}</h1>
        <WorkflowStateBadge state={run.state as 'RUNNING' | 'COMPLETE' | 'QUEUED'} />
      </div>
      <Tabs defaultValue="log">
        <TabsList>
          <TabsTrigger value="log">Log</TabsTrigger>
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
        <TabsContent value="lineage">
          <RunLineageTab runId={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
