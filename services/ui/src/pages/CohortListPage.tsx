import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { apiGet } from '@/api/client';
import { Plus, Users, Lock } from 'lucide-react';

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
  const { data, isLoading, error } = useQuery({
    queryKey: ['cohorts'],
    queryFn: () => apiGet<ListCohortsResponse>(`${COHORTS_BASE}/cohorts?limit=50`),
  });

  if (isLoading) return <div className="text-muted-foreground">Loading cohorts…</div>;
  if (error) return <div className="text-destructive">Failed to load cohorts: {String(error)}</div>;

  const cohorts = data?.cohorts ?? [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Cohort Browser</h1>
          <p className="text-muted-foreground">Define and manage sample cohorts with phenotype and DRS links.</p>
        </div>
        <Button asChild>
          <Link to={"/cohorts/new" as any}>
            <Plus className="mr-2 h-4 w-4" />
            New Cohort
          </Link>
        </Button>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Cohorts</CardTitle>
        </CardHeader>
        <CardContent>
          {cohorts.length === 0 ? (
            <p className="text-muted-foreground">No cohorts yet. Create one to get started.</p>
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
                            <Lock className="h-3 w-3" /> Frozen
                          </Badge>
                        )}
                      </div>
                      {c.description && (
                        <p className="text-sm text-muted-foreground">{c.description}</p>
                      )}
                    </div>
                  </div>
                  <div className="text-right text-sm text-muted-foreground">
                    {c.sample_count} samples · updated {new Date(c.updated_at).toLocaleDateString()}
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
