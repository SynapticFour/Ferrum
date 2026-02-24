import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ServiceHealthBadge } from '@/components/ServiceHealthBadge';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

interface RunSummary {
  run_id: string;
  state?: string;
}

interface RunListResponse {
  runs: RunSummary[];
  next_page_token?: string;
}

interface CostSummaryResponse {
  period: { from: string; to: string };
  total_runs: number;
  total_estimated_cost: { amount: number; currency: string };
  by_workflow_type: Record<string, number>;
  by_tag: Record<string, number>;
}

function last30DaysParams(): string {
  const to = new Date();
  const from = new Date(to);
  from.setDate(from.getDate() - 30);
  return `from_date=${encodeURIComponent(from.toISOString())}&to_date=${encodeURIComponent(to.toISOString())}`;
}

export function Dashboard() {
  const { data: runsData } = useQuery({
    queryKey: ['wes', 'runs', 'recent'],
    queryFn: () => apiGet<RunListResponse>('/ga4gh/wes/v1/runs?page_size=10'),
  });
  const recentRuns = runsData?.runs ?? [];

  const { data: costData } = useQuery({
    queryKey: ['wes', 'cost', 'summary', '30d'],
    queryFn: () => apiGet<CostSummaryResponse>(`/ga4gh/wes/v1/cost/summary?${last30DaysParams()}`),
  });

  const costByWorkflow = costData?.by_workflow_type
    ? Object.entries(costData.by_workflow_type).map(([name, value]) => ({ name, cost: value }))
    : [];

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">Overview of data, workflows, and system health.</p>
      </div>
      <div className="grid gap-4 md:grid-cols-5">
        <Card><CardHeader><CardTitle className="text-sm">DRS Objects</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Storage</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">—</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Active Runs</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">{recentRuns.filter((r) => r.state === 'RUNNING' || r.state === 'QUEUED').length}</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Est. cost (30d)</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">{costData ? `${costData.total_estimated_cost.currency} ${costData.total_estimated_cost.amount.toFixed(2)}` : '—'}</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Tools</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
      </div>
      {costByWorkflow.length > 0 && (
        <Card>
          <CardHeader><CardTitle>Cost by workflow type (last 30 days)</CardTitle></CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={260}>
              <BarChart data={costByWorkflow} margin={{ top: 8, right: 24, left: 8, bottom: 8 }}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="name" tick={{ fontSize: 11 }} />
                <YAxis tick={{ fontSize: 11 }} tickFormatter={(v) => `$${v}`} />
                <Tooltip formatter={(v: number) => [`${costData?.total_estimated_cost.currency ?? 'USD'} ${v.toFixed(2)}`, 'Cost']} />
                <Bar dataKey="cost" fill="#0ea5e9" name="Cost" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      )}
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
