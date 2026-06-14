import { Link, useParams } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { AddCohortSampleDialog } from '@/components/AddCohortSampleDialog';
import { SampleSheetImportDialog } from '@/components/SampleSheetImportDialog';
import { RunCohortDialog } from '@/components/RunCohortDialog';
import { apiGet, apiPost } from '@/api/client';
import { ArrowLeft, Users, Database, BarChart3 } from 'lucide-react';
import { formatBytes } from '@/lib/utils';
import { useI18n } from '@/i18n/I18nProvider';

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
  const { t } = useI18n();
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

  const stats = statsQuery.data;

  function sexLabel(sex: string) {
    return sex === 'not_recorded' ? t('cohort.sexNotRecorded') : sex;
  }

  function sortedSexEntries(entries: [string, number][]) {
    return [...entries].sort(([a], [b]) => {
      if (a === 'not_recorded') return 1;
      if (b === 'not_recorded') return -1;
      return a.localeCompare(b);
    });
  }

  const cohort = cohortQuery.data;
  const samples = samplesQuery.data?.samples ?? [];

  if (cohortQuery.isLoading || !cohort) {
    return <div className="text-muted-foreground">{t('cohortDetail.loading')}</div>;
  }
  if (cohortQuery.error) {
    return (
      <div className="text-destructive">
        {t('cohortDetail.failed')}: {String(cohortQuery.error)}
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
              <Badge variant="secondary">{t('cohortDetail.frozen')}</Badge>
            )}
            {!cohort.is_frozen && (
              <Button
                size="sm"
                variant="outline"
                onClick={async () => {
                  await apiPost(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId!)}/freeze`, {});
                  window.location.reload();
                }}
              >
                {t('cohortDetail.freeze')}
              </Button>
            )}
            <Button
              size="sm"
              variant="outline"
              onClick={async () => {
                const data = await apiGet<unknown>(
                  `${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId!)}/export`,
                );
                const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `cohort-${cohortId}.json`;
                a.click();
                URL.revokeObjectURL(url);
              }}
            >
              {t('cohortDetail.export')}
            </Button>
            {cohort.tags?.map((t) => (
              <Badge key={t} variant="outline">{t}</Badge>
            ))}
          </div>
        </div>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">{t('cohortDetail.tabOverview')}</TabsTrigger>
          <TabsTrigger value="samples">{t('cohortDetail.tabSamples')}</TabsTrigger>
        </TabsList>
        <TabsContent value="overview" className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">{t('cohortDetail.statSamples')}</CardTitle>
                <Users className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{cohort.sample_count}</div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">{t('cohortDetail.statDataSize')}</CardTitle>
                <Database className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats && stats.total_data_size_bytes > 0
                    ? formatBytes(stats.total_data_size_bytes)
                    : stats
                      ? '0 B'
                      : '—'}
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">{t('cohortDetail.statVersion')}</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{cohort.version}</div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium">{t('cohortDetail.statUpdated')}</CardTitle>
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
                  {t('cohortDetail.sexDistribution')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-wrap gap-4">
                  {sortedSexEntries(Object.entries(stats.sex_distribution)).map(([sex, count]) => (
                    <div key={sex} className="rounded bg-muted px-3 py-1">
                      <span className="font-medium">{sexLabel(sex)}</span>: {count}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </TabsContent>
        <TabsContent value="samples" className="space-y-4">
          <div className="flex flex-wrap justify-end gap-2">
            <SampleSheetImportDialog cohortId={cohortId!} disabled={cohort.is_frozen} />
            <RunCohortDialog
              cohortId={cohortId!}
              cohortName={cohort.name}
              workspaceId={cohort.workspace_id}
              disabled={cohort.is_frozen}
            />
            <AddCohortSampleDialog cohortId={cohortId!} disabled={cohort.is_frozen} />
          </div>
          <Card>
            <CardHeader>
              <CardTitle>{t('cohortDetail.samplesTitle')}</CardTitle>
              <p className="text-sm text-muted-foreground">
                {t('cohortDetail.samplesHint', { count: samples.length })}
              </p>
            </CardHeader>
            <CardContent>
              {samples.length === 0 ? (
                <p className="text-muted-foreground">{t('cohortDetail.noSamples')}</p>
              ) : (
                <div className="rounded-md border">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b bg-muted/50">
                        <th className="p-3 text-left font-medium">{t('cohortDetail.colSampleId')}</th>
                        <th className="p-3 text-left font-medium">{t('cohortDetail.colDrs')}</th>
                        <th className="p-3 text-left font-medium">{t('cohortDetail.colPhenotype')}</th>
                        <th className="p-3 text-left font-medium">{t('cohortDetail.colAdded')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {samples.map((s) => (
                        <tr key={s.id} className="border-b last:border-0">
                          <td className="p-3 font-mono">{s.sample_id}</td>
                          <td className="p-3">
                            {t('cohortDetail.objectCount', { count: s.drs_object_ids?.length ?? 0 })}
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
