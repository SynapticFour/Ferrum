import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { apiGet } from '@/api/client';
import { Play, AlertCircle } from 'lucide-react';

interface RunSummary {
  run_id: string;
  state?: string;
}

interface RunListResponse {
  runs: RunSummary[];
  next_page_token?: string;
}

export function WorkflowCenter() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['wes', 'runs'],
    queryFn: () => apiGet<RunListResponse>('/ga4gh/wes/v1/runs?page_size=20'),
    retry: false,
  });

  const runs = data?.runs ?? [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Workflow Center</h1>
          <p className="text-muted-foreground">Submit and monitor WES runs.</p>
        </div>
        <Button variant="outline" disabled className="gap-2">
          <Play className="h-4 w-4" />
          Submit workflow
        </Button>
      </div>
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          WES is not configured or unavailable. Configure the gateway with WES and TES to submit and list runs.
        </div>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Runs</CardTitle>
          <p className="text-sm text-muted-foreground">
            Submit workflows via the WES API (e.g. <code className="rounded bg-muted px-1">POST /ga4gh/wes/v1/runs</code>). Submit UI is coming in a future release.
          </p>
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
          {!isLoading && runs.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">No runs yet. Submit a run via the WES API to see it here.</p>
          )}
          {!isLoading && runs.length > 0 && (
            <ul className="space-y-2">
              {runs.map((r) => (
                <li key={r.run_id}>
                  <Link
                    to={`/workflows/runs/${r.run_id}` as any}
                    className="text-primary hover:underline font-mono text-sm"
                  >
                    {r.run_id}
                  </Link>
                  {r.state && <span className="text-muted-foreground ml-2 text-xs">({r.state})</span>}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
