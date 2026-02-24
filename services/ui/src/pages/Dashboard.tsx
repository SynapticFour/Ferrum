import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ServiceHealthBadge } from '@/components/ServiceHealthBadge';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';

interface RunSummary {
  run_id: string;
  state?: string;
}

interface RunListResponse {
  runs: RunSummary[];
  next_page_token?: string;
}

export function Dashboard() {
  const { data: runsData } = useQuery({
    queryKey: ['wes', 'runs', 'recent'],
    queryFn: () => apiGet<RunListResponse>('/ga4gh/wes/v1/runs?page_size=10'),
  });
  const recentRuns = runsData?.runs ?? [];

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">Overview of data, workflows, and system health.</p>
      </div>
      <div className="grid gap-4 md:grid-cols-4">
        <Card><CardHeader><CardTitle className="text-sm">DRS Objects</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Storage</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">—</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Active Runs</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">{recentRuns.filter((r) => r.state === 'RUNNING' || r.state === 'QUEUED').length}</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Tools</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
      </div>
      <Card>
        <CardHeader><CardTitle>Recent provenance</CardTitle></CardHeader>
        <CardContent>
          <p className="text-muted-foreground text-sm mb-2">Last 10 WES runs — open a run to view lineage.</p>
          <ul className="space-y-1">
            {recentRuns.slice(0, 10).map((r) => (
              <li key={r.run_id}>
                <Link
                  to="/workflows/runs/$runId"
                  params={{ runId: r.run_id }}
                  className="text-primary hover:underline font-mono text-sm"
                >
                  {r.run_id}
                </Link>
                {r.state && <span className="text-muted-foreground ml-2 text-xs">({r.state})</span>}
              </li>
            ))}
            {recentRuns.length === 0 && <li className="text-muted-foreground text-sm">No runs yet.</li>}
          </ul>
        </CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle>System health</CardTitle></CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          <ServiceHealthBadge status="up" label="Gateway" />
          <ServiceHealthBadge status="up" label="DRS" />
          <ServiceHealthBadge status="up" label="WES" />
        </CardContent>
      </Card>
    </div>
  );
}
