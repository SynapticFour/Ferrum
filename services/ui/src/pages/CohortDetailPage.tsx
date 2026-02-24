import { Link, useParams } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { apiGet } from '@/api/client';
import { ArrowLeft, Users, Database, BarChart3 } from 'lucide-react';

const COHORTS_BASE = '/cohorts/v1';

type CohortDetail = {
  id: string;
  name: string;
  description: string | null;
  owner_sub: string;
  workspace_id: string | null;
  version: number;
  is_frozen: boolean;
  sample_count: number;
  tags: string[];
  filter_criteria: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type CohortSample = {
  id: string;
  cohort_id: string;
  sample_id: string;
  drs_object_ids: string[];
  phenotype: Record<string, unknown>;
  added_at: string;
  added_by: string;
};

type ListSamplesResponse = {
  samples: CohortSample[];
  next_offset: number | null;
};

type CohortStats = {
  cohort_id: string;
  sample_count: number;
  total_data_size_bytes: number;
  data_type_breakdown: Record<string, { count: number; total_size: number; mime_type: string }>;
  phenotype_completeness: Record<string, number>;
  sex_distribution: Record<string, number>;
};

export function CohortDetailPage() {
  const params = useParams({ strict: false }) as { cohortId?: string };
  const cohortId = params.cohortId;
  const cohortQuery = useQuery({
    queryKey: ['cohort', cohortId],
    queryFn: () => apiGet<CohortDetail>(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId!)}`),
    enabled: !!cohortId,
  });
  const samplesQuery = useQuery({
    queryKey: ['cohort-samples', cohortId],
    queryFn: () =>
      apiGet<ListSamplesResponse>(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId!)}/samples?limit=100`),
    enabled: !!cohortId,
  });
  const statsQuery = useQuery({
    queryKey: ['cohort-stats', cohortId],
    queryFn: () => apiGet<CohortStats>(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId!)}/stats`),
    enabled: !!cohortId,
  });

  const cohort = cohortQuery.data;
  const samples = samplesQuery.data?.samples ?? [];
  const stats = statsQuery.data;

  if (cohortQuery.isLoading || !cohort) {
    return <div className="text-muted-foreground">Loading cohort…</div>;
  }
  if (cohortQuery.error) {
    return (
      <div className="text-destructive">
        Failed to load cohort: {String(cohortQuery.error)}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link to={"/cohorts" as any}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">{cohort.name}</h1>
          {cohort.description && (
            <p className="text-muted-foreground">{cohort.description}</p>
          )}
          <div className="mt-2 flex items-center gap-2">
            {cohort.is_frozen && (
              <Badge variant="secondary">Frozen</Badge>
            )}
            {cohort.tags?.map((t) => (
              <Badge key={t} variant="outline">{t}</Badge>
            ))}
          </div>
        </div>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="samples">Samples</TabsTrigger>
        </TabsList>
        <TabsContent value="overview" className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">Samples</CardTitle>
                <Users className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{cohort.sample_count}</div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">Data size</CardTitle>
                <Database className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats ? `${(stats.total_data_size_bytes / 1e9).toFixed(2)} GB` : '—'}
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">Version</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{cohort.version}</div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">Last updated</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-sm">{new Date(cohort.updated_at).toLocaleString()}</div>
              </CardContent>
            </Card>
          </div>
          {stats && Object.keys(stats.sex_distribution).length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <BarChart3 className="h-5 w-5" />
                  Sex distribution
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-wrap gap-4">
                  {Object.entries(stats.sex_distribution).map(([sex, count]) => (
                    <div key={sex} className="rounded bg-muted px-3 py-1">
                      <span className="font-medium">{sex}</span>: {count}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </TabsContent>
        <TabsContent value="samples" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Samples</CardTitle>
              <p className="text-sm text-muted-foreground">
                {samples.length} sample(s). Add or import via API.
              </p>
            </CardHeader>
            <CardContent>
              {samples.length === 0 ? (
                <p className="text-muted-foreground">No samples in this cohort yet.</p>
              ) : (
                <div className="rounded-md border">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b bg-muted/50">
                        <th className="p-3 text-left font-medium">Sample ID</th>
                        <th className="p-3 text-left font-medium">DRS objects</th>
                        <th className="p-3 text-left font-medium">Phenotype</th>
                        <th className="p-3 text-left font-medium">Added</th>
                      </tr>
                    </thead>
                    <tbody>
                      {samples.map((s) => (
                        <tr key={s.id} className="border-b last:border-0">
                          <td className="p-3 font-mono">{s.sample_id}</td>
                          <td className="p-3">
                            {s.drs_object_ids?.length ?? 0} object(s)
                          </td>
                          <td className="p-3 max-w-xs truncate">
                            {Object.keys(s.phenotype ?? {}).length > 0
                              ? JSON.stringify(s.phenotype)
                              : '—'}
                          </td>
                          <td className="p-3 text-muted-foreground">
                            {new Date(s.added_at).toLocaleDateString()}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
