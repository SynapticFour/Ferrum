import { Link, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { ArrowLeft, FolderOpen, Play, Users, BookOpen } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';
import { formatBytes } from '@/lib/utils';
import { WorkspaceMembersPanel } from '@/components/WorkspaceMembersPanel';
import { WorkspaceSettingsForm } from '@/components/WorkspaceSettingsForm';
import { ImportToDrsDialog } from '@/components/ImportToDrsDialog';
import { LinkWorkspaceDataDialog } from '@/components/LinkWorkspaceDataDialog';
import { StartAnalysisDialog } from '@/components/StartAnalysisDialog';

import type { DrsObject } from '@/api/types';

interface CohortSummary {
  id: string;
  name: string;
  sample_count: number;
}

interface RunSummary {
  run_id: string;
  state?: string;
}

interface Workspace {
  id: string;
  name: string;
  description: string | null;
  slug: string;
  owner_sub: string;
  is_archived: boolean;
}

interface ContentSummary {
  count: number;
  recent: unknown[];
}
interface WorkspaceContents {
  drs_objects: ContentSummary;
  wes_runs: ContentSummary;
  cohorts: ContentSummary;
  total_size_bytes: number;
  active_runs: number;
}
interface ActivityItem {
  id: string;
  workspace_id: string;
  sub: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  occurred_at: string | null;
}

export function WorkspaceDetailPage() {
  const { t } = useI18n();
  const params = useParams({ strict: false }) as { workspaceId?: string };
  const id = params.workspaceId ?? '';

  const { data: workspaceCohorts } = useQuery({
    queryKey: ['cohorts', 'workspace', id],
    queryFn: () =>
      apiGet<{ cohorts: CohortSummary[] }>(
        `/cohorts/v1/cohorts?workspace_id=${encodeURIComponent(id)}&limit=20`,
      ),
    enabled: !!id,
  });

  const { data: workspaceRuns } = useQuery({
    queryKey: ['wes', 'runs', 'workspace', id],
    queryFn: () =>
      apiGet<{ runs: RunSummary[] }>(
        `/ga4gh/wes/v1/runs?workspace_id=${encodeURIComponent(id)}&page_size=20`,
      ),
    enabled: !!id,
  });

  const { data: workspace, isLoading, error } = useQuery({
    queryKey: ['workspace', id],
    queryFn: () => apiGet<Workspace>(`/workspaces/v1/workspaces/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  const { data: contents } = useQuery({
    queryKey: ['workspace', id, 'contents'],
    queryFn: () => apiGet<WorkspaceContents>(`/workspaces/v1/workspaces/${encodeURIComponent(id)}/contents`),
    enabled: !!id,
  });

  const { data: workspaceObjects } = useQuery({
    queryKey: ['drs', 'objects', 'workspace', id],
    queryFn: () =>
      apiGet<DrsObject[]>(`/ga4gh/drs/v1/objects?workspace_id=${encodeURIComponent(id)}&limit=20`),
    enabled: !!id,
  });

  const { data: activity } = useQuery({
    queryKey: ['workspace', id, 'activity'],
    queryFn: () => apiGet<ActivityItem[]>(`/workspaces/v1/workspaces/${encodeURIComponent(id)}/activity`),
    enabled: !!id,
  });

  if (!id) return <p className="text-muted-foreground">{t('workspace.noId')}</p>;
  if (isLoading) return <p className="text-muted-foreground">{t('common.loading')}</p>;
  if (error || !workspace) return <p className="text-destructive">{t('workspace.notFound')}</p>;

  const objects = Array.isArray(workspaceObjects) ? workspaceObjects : [];

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={'/workspaces' as any}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-2xl font-bold">{workspace.name}</h1>
          {workspace.description && <p className="text-muted-foreground">{workspace.description}</p>}
        </div>
        <div className="ml-auto flex flex-wrap gap-2">
          <StartAnalysisDialog workspaceId={id} />
        </div>
      </div>

      <Card className="border-primary/20 bg-primary/5">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <BookOpen className="h-4 w-4" />
            {t('workspace.purpose')}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-sm text-muted-foreground">
          <p>{t('workspace.purposeBody')}</p>
          <p className="text-xs">{t('workspace.demoNote')}</p>
          <ol className="list-decimal list-inside space-y-1 pt-1">
            <li>{t('workspace.step1')}</li>
            <li>{t('workspace.step2')}</li>
            <li>{t('workspace.step3')}</li>
            <li>{t('workspace.step4')}</li>
          </ol>
        </CardContent>
      </Card>

      <Tabs defaultValue="overview">
        <TabsList className="grid w-full grid-cols-4 lg:grid-cols-7">
          <TabsTrigger value="overview">{t('workspace.tabOverview')}</TabsTrigger>
          <TabsTrigger value="data">{t('workspace.tabData')}</TabsTrigger>
          <TabsTrigger value="workflows">{t('workspace.tabWorkflows')}</TabsTrigger>
          <TabsTrigger value="cohorts">{t('workspace.tabCohorts')}</TabsTrigger>
          <TabsTrigger value="members" className="hidden lg:inline-flex">{t('workspace.tabMembers')}</TabsTrigger>
          <TabsTrigger value="activity" className="hidden lg:inline-flex">{t('workspace.tabActivity')}</TabsTrigger>
          <TabsTrigger value="settings" className="hidden lg:inline-flex">{t('workspace.tabSettings')}</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t('workspace.objects')}</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents?.drs_objects?.count ?? 0}</span></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t('workspace.totalSize')}</CardTitle></CardHeader>
              <CardContent>
                <span className="text-2xl font-bold">
                  {contents && contents.total_size_bytes > 0 ? formatBytes(contents.total_size_bytes) : '—'}
                </span>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t('workspace.runs')}</CardTitle></CardHeader>
              <CardContent>
                <span className="text-2xl font-bold">{contents?.wes_runs?.count ?? 0}</span>
                <span className="text-muted-foreground text-sm ml-1">
                  ({contents?.active_runs ?? 0} {t('workspace.active')})
                </span>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t('workspace.cohorts')}</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents?.cohorts?.count ?? 0}</span></CardContent>
            </Card>
          </div>
          <Card className="mt-4">
            <CardHeader><CardTitle>{t('workspace.gettingStarted')}</CardTitle></CardHeader>
            <CardContent className="flex flex-wrap gap-2">
              <StartAnalysisDialog workspaceId={id} />
              <Button asChild variant="outline" className="gap-1">
                <Link to={'/data' as any}><FolderOpen className="h-4 w-4" />{t('workspace.openData')}</Link>
              </Button>
              <Button asChild variant="outline" className="gap-1">
                <Link to={'/workflows' as any}><Play className="h-4 w-4" />{t('workspace.openWorkflows')}</Link>
              </Button>
              <Button asChild variant="outline" className="gap-1">
                <Link to={'/cohorts' as any}><Users className="h-4 w-4" />{t('workspace.openCohorts')}</Link>
              </Button>
            </CardContent>
          </Card>
          <Card className="mt-4">
            <CardHeader><CardTitle>{t('workspace.recentActivity')}</CardTitle></CardHeader>
            <CardContent>
              <ul className="space-y-2 text-sm">
                {(Array.isArray(activity) ? activity.slice(0, 20) : []).map((item, i) => (
                  <li key={item.id ?? i}>
                    <span className="font-medium">{item.sub}</span> {item.action}
                    {item.resource_type && <span className="text-muted-foreground"> · {item.resource_type}</span>}
                    <span className="text-muted-foreground ml-2">{item.occurred_at ? new Date(item.occurred_at).toLocaleString() : ''}</span>
                  </li>
                ))}
                {(!Array.isArray(activity) || !activity.length) && <li className="text-muted-foreground">{t('workspace.noActivity')}</li>}
              </ul>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="data">
          <Card>
            <CardHeader>
              <CardTitle>{t('workspace.dataInWorkspace')}</CardTitle>
              <p className="text-sm text-muted-foreground">{t('workspace.dataHint')}</p>
            </CardHeader>
            <CardContent>
              <div className="flex flex-wrap gap-2 mb-4">
                <ImportToDrsDialog linkToWorkspaceId={id} triggerVariant="default" />
                <LinkWorkspaceDataDialog workspaceId={id} />
              </div>
              {objects.length > 0 ? (
                <ul className="space-y-2 text-sm mb-4">
                  {objects.map((o) => (
                    <li key={o.id} className="flex justify-between gap-2 border-b border-border/50 pb-2">
                      <div className="flex flex-wrap items-center gap-2 min-w-0">
                        <Link to={`/data/objects/${o.id}` as any} className="font-medium hover:underline truncate">
                          {o.name ?? o.id}
                        </Link>
                        <Link
                          to={`/data/objects/${o.id}?analyze=1` as any}
                          className="text-xs text-primary hover:underline shrink-0"
                        >
                          {t('object.useInAnalysis')}
                        </Link>
                      </div>
                      <span className="text-muted-foreground shrink-0">
                        {o.size != null && o.size > 0 ? formatBytes(o.size) : '—'}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-muted-foreground text-sm mb-4">{t('workspace.noData')}</p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="workflows">
          <Card>
            <CardHeader>
              <CardTitle>{t('workspace.workflowsInWorkspace')}</CardTitle>
              <p className="text-muted-foreground text-sm">{t('workspace.workflowsHint')}</p>
            </CardHeader>
            <CardContent>
              <div className="mb-4">
                <StartAnalysisDialog workspaceId={id} />
              </div>
              {(workspaceRuns?.runs ?? []).length > 0 ? (
                <ul className="space-y-2 text-sm mb-4">
                  {workspaceRuns!.runs.map((r) => (
                    <li key={r.run_id} className="flex justify-between border-b border-border/50 pb-2">
                      <Link to={`/workflows/runs/${r.run_id}` as any} className="font-mono hover:underline">
                        {r.run_id.slice(0, 12)}…
                      </Link>
                      <span className="text-muted-foreground">{r.state ?? '—'}</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground mb-4">{t('workspace.noRuns')}</p>
              )}
              <Button asChild className="gap-1">
                <Link to={'/workflows' as any}><Play className="h-4 w-4" />{t('workspace.openWorkflows')}</Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="cohorts">
          <Card>
            <CardHeader><CardTitle>{t('workspace.cohortsInWorkspace')}</CardTitle></CardHeader>
            <CardContent>
              {(workspaceCohorts?.cohorts ?? []).length > 0 ? (
                <ul className="space-y-2 text-sm mb-4">
                  {workspaceCohorts!.cohorts.map((c) => (
                    <li key={c.id} className="flex justify-between border-b border-border/50 pb-2">
                      <Link to={`/cohorts/${c.id}` as any} className="font-medium hover:underline">{c.name}</Link>
                      <span className="text-muted-foreground">{c.sample_count} samples</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground mb-4">{t('workspace.noCohorts')}</p>
              )}
              <Button asChild>
                <Link to={'/cohorts' as any}>{t('workspace.openCohorts')}</Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="members">
          <WorkspaceMembersPanel workspaceId={id} />
        </TabsContent>
        <TabsContent value="activity">
          <Card>
            <CardHeader><CardTitle>{t('workspace.tabActivity')}</CardTitle></CardHeader>
            <CardContent>
              <ul className="space-y-2 text-sm">
                {(Array.isArray(activity) ? activity : []).map((item, i) => (
                  <li key={item.id ?? i}>
                    <span className="font-medium">{item.sub}</span> {item.action}
                    {item.resource_type && <span className="text-muted-foreground"> · {item.resource_type}</span>}
                    <span className="text-muted-foreground ml-2">{item.occurred_at ? new Date(item.occurred_at).toLocaleString() : ''}</span>
                  </li>
                ))}
                {(!Array.isArray(activity) || !activity.length) && <li className="text-muted-foreground">{t('workspace.noActivity')}</li>}
              </ul>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="settings">
          <WorkspaceSettingsForm workspaceId={id} name={workspace.name} description={workspace.description} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
