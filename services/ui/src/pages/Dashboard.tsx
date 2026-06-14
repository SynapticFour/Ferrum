import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { ServiceHealthPanel } from '@/components/ServiceHealthPanel';
import { MetricTile } from '@/components/MetricTile';
import { useI18n, useRunStateLabel } from '@/i18n/I18nProvider';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
} from 'recharts';
import {
  Database,
  Workflow,
  Users,
  FolderOpen,
  ArrowRight,
  Activity,
  DollarSign,
  FlaskConical,
  Shield,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { DeploymentModeBanner } from '@/components/DeploymentModeBanner';
import { NoopExecutorBanner } from '@/components/NoopExecutorBanner';

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

interface Workspace {
  id: string;
  name: string;
  description: string | null;
  slug: string;
}

interface ListCohortsResponse {
  cohorts: { id: string }[];
  next_offset: number | null;
}

function last30DaysParams(): string {
  const to = new Date();
  const from = new Date(to);
  from.setDate(from.getDate() - 30);
  return `from_date=${encodeURIComponent(from.toISOString())}&to_date=${encodeURIComponent(to.toISOString())}`;
}

const RUN_STATE_COLORS: Record<string, string> = {
  RUNNING: 'hsl(142 76% 36%)',
  QUEUED: 'hsl(38 92% 50%)',
  COMPLETE: 'hsl(199 89% 48%)',
  EXECUTOR_ERROR: 'hsl(0 84% 60%)',
  SYSTEM_ERROR: 'hsl(0 84% 60%)',
  CANCELED: 'hsl(215 16% 57%)',
  UNKNOWN: 'hsl(215 16% 57%)',
};


export function Dashboard() {
  const { t } = useI18n();
  const labelRunState = useRunStateLabel();
  const { data: runsData, isLoading: runsLoading } = useQuery({
    queryKey: ['wes', 'runs', 'recent'],
    queryFn: () => apiGet<RunListResponse>('/ga4gh/wes/v1/runs?page_size=20'),
  });
  const recentRuns = runsData?.runs ?? [];

  const { data: costData } = useQuery({
    queryKey: ['wes', 'cost', 'summary', '30d'],
    queryFn: () => apiGet<CostSummaryResponse>(`/ga4gh/wes/v1/cost/summary?${last30DaysParams()}`),
  });

  const { data: workspaces = [] } = useQuery({
    queryKey: ['workspaces'],
    queryFn: () => apiGet<Workspace[]>('/workspaces/v1/workspaces'),
  });

  const { data: cohortsData } = useQuery({
    queryKey: ['cohorts', 'count'],
    queryFn: () => apiGet<ListCohortsResponse>('/cohorts/v1/cohorts?limit=500'),
  });
  const cohortCount = cohortsData?.cohorts?.length ?? 0;

  const costByWorkflow = costData?.by_workflow_type
    ? Object.entries(costData.by_workflow_type).map(([name, value]) => ({ name, cost: value }))
    : [];

  const runStateCounts = recentRuns.reduce<Record<string, number>>((acc, r) => {
    const s = r.state ?? 'UNKNOWN';
    acc[s] = (acc[s] ?? 0) + 1;
    return acc;
  }, {});
  const runStateChartData = Object.entries(runStateCounts).map(([state, value]) => ({
    name: labelRunState(state),
    value,
    color: RUN_STATE_COLORS[state] ?? 'hsl(215 16% 57%)',
  }));

  const activeRuns = recentRuns.filter((r) => r.state === 'RUNNING' || r.state === 'QUEUED').length;
  const totalRuns30d = costData?.total_runs ?? recentRuns.length;
  const hasActivity = recentRuns.length > 0 || workspaces.length > 0 || cohortCount > 0;
  const primaryWorkspaceId = workspaces[0]?.id;

  return (
    <div className="space-y-10">
      <DeploymentModeBanner />
      <NoopExecutorBanner />

      {/* Hero + Quick actions */}
      <section className="relative rounded-2xl border border-border/60 bg-gradient-to-br from-card via-card to-primary/5 p-8">
        <div className="flex flex-col gap-6 md:flex-row md:items-center md:justify-between">
          <div>
            <h1 className="text-3xl font-bold tracking-tight text-foreground md:text-4xl">
              {t('dashboard.welcome')}
            </h1>
            <p className="mt-2 max-w-xl text-muted-foreground">{t('dashboard.subtitle')}</p>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button asChild variant="default" size="sm" className="gap-2">
              <Link to={"/data" as any}>
                <Database className="h-4 w-4" />
                {t('dashboard.browseData')}
              </Link>
            </Button>
            <Button asChild variant="outline" size="sm" className="gap-2">
              <Link to={"/workflows" as any}>
                <Workflow className="h-4 w-4" />
                {t('nav.workflows')}
              </Link>
            </Button>
            <Button asChild variant="outline" size="sm" className="gap-2">
              <Link to={"/cohorts/new" as any}>
                <Users className="h-4 w-4" />
                {t('dashboard.newCohort')}
              </Link>
            </Button>
            <Button asChild variant="outline" size="sm" className="gap-2">
              <Link to={"/workspaces/new" as any}>
                <FolderOpen className="h-4 w-4" />
                {t('dashboard.newWorkspace')}
              </Link>
            </Button>
          </div>
        </div>
      </section>

      {/* Key metrics */}
      <section>
        <h2 className="mb-4 text-lg font-semibold text-foreground">{t('dashboard.atAGlance')}</h2>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
          <MetricTile
            to={primaryWorkspaceId ? `/workspaces/${primaryWorkspaceId}` : '/workspaces'}
            label={t('nav.workspaces')}
            value={workspaces.length}
            icon={FolderOpen}
            subtitle={workspaces[0]?.name}
          />
          <MetricTile
            to="/cohorts"
            label={t('nav.cohorts')}
            value={cohortCount}
            icon={Users}
          />
          <MetricTile
            to="/workflows"
            label={t('dashboard.activeRuns')}
            value={activeRuns}
            icon={Activity}
            iconClassName="bg-emerald-500/10 text-emerald-500"
          />
          <MetricTile
            to="/workflows"
            label={t('dashboard.runs30d')}
            value={runsLoading ? '…' : totalRuns30d}
            icon={Workflow}
          />
          <MetricTile
            to="/workflows"
            label={t('dashboard.estCost30d')}
            value={
              costData
                ? `${costData.total_estimated_cost.currency} ${costData.total_estimated_cost.amount.toFixed(2)}`
                : '—'
            }
            icon={DollarSign}
          />
        </div>
      </section>

      <div className="grid gap-8 lg:grid-cols-3">
        {/* Recent runs + run state */}
        <section className="lg:col-span-2 space-y-6">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="flex items-center gap-2 text-base">
                <Activity className="h-4 w-4" />
                {t('dashboard.recentRuns')}
              </CardTitle>
              <p className="text-sm text-muted-foreground">{t('dashboard.recentRunsHint')}</p>
            </CardHeader>
            <CardContent>
              {recentRuns.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border/60 bg-muted/20 py-10 text-center text-sm text-muted-foreground">
                  {t('dashboard.noRuns')}
                </div>
              ) : (
                <ul className="space-y-2">
                  {recentRuns.slice(0, 10).map((r) => (
                    <li key={r.run_id}>
                      <Link
                        to={`/workflows/runs/${r.run_id}` as any}
                        className="flex items-center justify-between rounded-lg border border-transparent px-3 py-2 transition-colors hover:border-border hover:bg-muted/50"
                      >
                        <span className="font-mono text-sm text-foreground truncate max-w-[60%]">
                          {r.run_id}
                        </span>
                        <span
                          className={cn(
                            'rounded-full px-2 py-0.5 text-xs font-medium',
                            r.state === 'RUNNING' && 'bg-emerald-500/20 text-emerald-400',
                            r.state === 'QUEUED' && 'bg-amber-500/20 text-amber-400',
                            r.state === 'COMPLETE' && 'bg-primary/20 text-primary',
                            (r.state === 'EXECUTOR_ERROR' || r.state === 'SYSTEM_ERROR') && 'bg-red-500/20 text-red-400',
                            !r.state || (r.state !== 'RUNNING' && r.state !== 'QUEUED' && r.state !== 'COMPLETE' && r.state !== 'EXECUTOR_ERROR' && r.state !== 'SYSTEM_ERROR') && 'bg-muted text-muted-foreground'
                          )}
                        >
                          {labelRunState(r.state ?? 'UNKNOWN')}
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              )}
              {recentRuns.length > 0 && (
                <div className="mt-4">
                  <Button asChild variant="ghost" size="sm" className="gap-1 text-muted-foreground">
                    <Link to={"/workflows" as any}>
                      {t('dashboard.viewAllRuns')}
                      <ArrowRight className="h-3 w-3" />
                    </Link>
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>

          {runStateChartData.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">{t('dashboard.runStates')}</CardTitle>
              </CardHeader>
              <CardContent>
                <ResponsiveContainer width="100%" height={200}>
                  <PieChart>
                    <Pie
                      data={runStateChartData}
                      cx="50%"
                      cy="50%"
                      innerRadius={50}
                      outerRadius={80}
                      paddingAngle={2}
                      dataKey="value"
                      nameKey="name"
                      label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`}
                    >
                      {runStateChartData.map((entry, i) => (
                        <Cell key={i} fill={entry.color} />
                      ))}
                    </Pie>
                    <Tooltip formatter={(value: number) => [value, 'Runs']} />
                  </PieChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          )}

          {costByWorkflow.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">{t('dashboard.costByWorkflow')}</CardTitle>
              </CardHeader>
              <CardContent>
                <ResponsiveContainer width="100%" height={260}>
                  <BarChart data={costByWorkflow} margin={{ top: 8, right: 24, left: 8, bottom: 8 }}>
                    <CartesianGrid strokeDasharray="3 3" className="stroke-border/50" />
                    <XAxis dataKey="name" tick={{ fontSize: 11 }} className="text-muted-foreground" />
                    <YAxis tick={{ fontSize: 11 }} tickFormatter={(v) => `$${v}`} className="text-muted-foreground" />
                    <Tooltip
                      formatter={(v: number) => [`${costData?.total_estimated_cost.currency ?? 'USD'} ${v.toFixed(2)}`, 'Cost']}
                      contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: 'var(--radius)' }}
                    />
                    <Bar dataKey="cost" fill="hsl(var(--primary))" name="Cost" radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          )}
        </section>

        {/* Sidebar: Quick links + Health */}
        <aside className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <FlaskConical className="h-4 w-4" />
                {t('dashboard.quickLinks')}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <Link
                to={"/data" as any}
                className="flex items-center justify-between rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t('dashboard.dataBrowser')}
                <ArrowRight className="h-3 w-3" />
              </Link>
              <Link
                to={"/workflows" as any}
                className="flex items-center justify-between rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t('dashboard.workflowRuns')}
                <ArrowRight className="h-3 w-3" />
              </Link>
              <Link
                to={"/tools" as any}
                className="flex items-center justify-between rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t('dashboard.toolRegistry')}
                <ArrowRight className="h-3 w-3" />
              </Link>
              <Link
                to={"/beacon" as any}
                className="flex items-center justify-between rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t('dashboard.beaconDiscovery')}
                <ArrowRight className="h-3 w-3" />
              </Link>
              <Link
                to={"/settings" as any}
                className="flex items-center justify-between rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t('nav.settings')}
                <ArrowRight className="h-3 w-3" />
              </Link>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Shield className="h-4 w-4" />
                {t('dashboard.systemHealth')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ServiceHealthPanel />
            </CardContent>
          </Card>

          {!hasActivity && (
            <Card className="border-dashed border-primary/30 bg-primary/5">
              <CardContent className="pt-6">
                <p className="text-sm font-medium text-foreground">{t('dashboard.getStarted')}</p>
                <p className="mt-1 text-xs text-muted-foreground">{t('dashboard.getStartedHint')}</p>
                <div className="mt-4 flex flex-wrap gap-2">
                  <Button asChild size="sm">
                    <Link to={'/study/setup' as any}>{t('dashboard.studySetup')}</Link>
                  </Button>
                  <Button asChild size="sm" variant="outline">
                    <Link to={'/workspaces/new' as any}>{t('dashboard.newWorkspace')}</Link>
                  </Button>
                  <Button asChild size="sm" variant="outline">
                    <Link to={'/workflows' as any}>{t('dashboard.runWorkflow')}</Link>
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}
        </aside>
      </div>
    </div>
  );
}
