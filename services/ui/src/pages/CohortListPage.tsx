import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { apiGet } from '@/api/client';
import { Plus, Users, Lock } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { pickSeedHintKey } from '@/lib/pilotContext';

const COHORTS_BASE = '/cohorts/v1';

export type CohortSummary = {
  id: string;
  name: string;
  description: string | null;
  owner_sub: string;
  workspace_id: string | null;
  version: number;
  is_frozen: boolean;
  sample_count: number;
  tags: string[];
  created_at: string;
  updated_at: string;
};

type ListCohortsResponse = {
  cohorts: CohortSummary[];
  next_offset: number | null;
};

export function CohortListPage() {
  const { t } = useI18n();
  const { data: adminConfig } = useAdminConfig();
  const { data, isLoading, error } = useQuery({
    queryKey: ['cohorts'],
    queryFn: () => apiGet<ListCohortsResponse>(`${COHORTS_BASE}/cohorts?limit=50`),
  });

  if (isLoading) return <div className="text-muted-foreground">{t('cohortList.loading')}</div>;
  if (error) return <div className="text-destructive">{t('cohortList.failed')}: {String(error)}</div>;

  const cohorts = data?.cohorts ?? [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('cohortList.title')}</h1>
          <p className="text-muted-foreground">{t('cohortList.subtitle')}</p>
        </div>
        <Button asChild>
          <Link to={"/cohorts/new" as any}>
            <Plus className="mr-2 h-4 w-4" />
            {t('cohortList.new')}
          </Link>
        </Button>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>{t('cohortList.cardTitle')}</CardTitle>
        </CardHeader>
        <CardContent>
          {cohorts.length === 0 ? (
            <div className="space-y-3 text-sm">
              <p className="font-medium">{t('cohortList.emptyTitle')}</p>
              <p className="text-muted-foreground">{t('cohortList.emptyBody')}</p>
              <p className="text-muted-foreground border rounded-md p-3 bg-muted/30">
                {t(pickSeedHintKey(adminConfig, 'cohortList.emptySeedHint'))}
              </p>
              <div className="flex flex-wrap gap-2 pt-1">
                <Button asChild variant="outline" size="sm">
                  <Link to={'/study/setup' as any}>{t('cohortList.emptyStudyLink')}</Link>
                </Button>
                <Button asChild size="sm">
                  <Link to={'/cohorts/new' as any}>{t('cohortList.new')}</Link>
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              {cohorts.map((c) => (
                <Link
                  key={c.id}
                  to={"/cohorts/" + c.id}
                  className="flex items-center justify-between rounded-lg border p-4 transition-colors hover:bg-muted/50"
                >
                  <div className="flex items-center gap-3">
                    <Users className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <div className="font-medium flex items-center gap-2">
                        {c.name}
                        {c.is_frozen && (
                          <Badge variant="secondary" className="gap-1">
                            <Lock className="h-3 w-3" /> {t('cohortList.frozen')}
                          </Badge>
                        )}
                      </div>
                      {c.description && (
                        <p className="text-sm text-muted-foreground">{c.description}</p>
                      )}
                    </div>
                  </div>
                  <div className="text-right text-sm text-muted-foreground">
                    {t('cohortList.samplesUpdated', {
                      count: c.sample_count,
                      date: new Date(c.updated_at).toLocaleDateString(),
                    })}
                  </div>
                </Link>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
