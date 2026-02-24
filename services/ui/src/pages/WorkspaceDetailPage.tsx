import { Link, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { ArrowLeft } from 'lucide-react';

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
  const params = useParams({ strict: false }) as { workspaceId?: string };
  const id = params.workspaceId ?? '';

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

  const { data: activity } = useQuery({
    queryKey: ['workspace', id, 'activity'],
    queryFn: () => apiGet<ActivityItem[]>(`/workspaces/v1/workspaces/${encodeURIComponent(id)}/activity`),
    enabled: !!id,
  });

  if (!id) return <p className="text-muted-foreground">No workspace.</p>;
  if (isLoading) return <p className="text-muted-foreground">Loading…</p>;
  if (error || !workspace) return <p className="text-destructive">Workspace not found.</p>;

  const formatBytes = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} GB` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${(n / 1e3).toFixed(1)} KB`);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={"/workspaces" as any}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-2xl font-bold">{workspace.name}</h1>
          {workspace.description && <p className="text-muted-foreground">{workspace.description}</p>}
        </div>
      </div>
      <Tabs defaultValue="overview">
        <TabsList className="grid w-full grid-cols-7">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="data">Data</TabsTrigger>
          <TabsTrigger value="workflows">Workflows</TabsTrigger>
          <TabsTrigger value="cohorts">Cohorts</TabsTrigger>
          <TabsTrigger value="members">Members</TabsTrigger>
          <TabsTrigger value="activity">Activity</TabsTrigger>
          <TabsTrigger value="settings">Settings</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Objects</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents?.drs_objects?.count ?? 0}</span></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total size</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents ? formatBytes(contents.total_size_bytes) : '—'}</span></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Runs</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents?.wes_runs?.count ?? 0}</span> <span className="text-muted-foreground text-sm">({contents?.active_runs ?? 0} active)</span></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Cohorts</CardTitle></CardHeader>
              <CardContent><span className="text-2xl font-bold">{contents?.cohorts?.count ?? 0}</span></CardContent>
            </Card>
          </div>
          <Card className="mt-4">
            <CardHeader><CardTitle>Recent activity</CardTitle></CardHeader>
            <CardContent>
              <ul className="space-y-2 text-sm">
                {(Array.isArray(activity) ? activity.slice(0, 20) : []).map((item, i) => (
                  <li key={item.id ?? i}>
                    <span className="font-medium">{item.sub}</span> {item.action}
                    {item.resource_type && <span className="text-muted-foreground"> · {item.resource_type}</span>}
                    <span className="text-muted-foreground ml-2">{item.occurred_at ? new Date(item.occurred_at).toLocaleString() : ''}</span>
                  </li>
                ))}
                {(!Array.isArray(activity) || !activity.length) && <li className="text-muted-foreground">No activity yet.</li>}
              </ul>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="data">
          <Card>
            <CardHeader><CardTitle>Data (DRS)</CardTitle></CardHeader>
            <CardContent>
              <p className="text-muted-foreground">DRS objects in this workspace. Use Data Browser with workspace filter.</p>
              <Button asChild className="mt-4">
                <Link to={"/data" as any} search={{ workspace_id: id } as any}>Open Data Browser</Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="workflows">
          <Card>
            <CardHeader><CardTitle>Workflows</CardTitle></CardHeader>
            <CardContent>
              <p className="text-muted-foreground">WES runs in this workspace.</p>
              <Button asChild className="mt-4">
                <Link to={"/workflows" as any} search={{ workspace_id: id } as any}>Open Workflow Center</Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="cohorts">
          <Card>
            <CardHeader><CardTitle>Cohorts</CardTitle></CardHeader>
            <CardContent>
              <Button asChild>
                <Link to={"/cohorts" as any} search={{ workspace_id: id } as any}>Browse cohorts</Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="members">
          <Card>
            <CardHeader><CardTitle>Members</CardTitle></CardHeader>
            <CardContent><p className="text-muted-foreground">Member list and invites. Use Members API.</p></CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="activity">
          <Card>
            <CardHeader><CardTitle>Activity log</CardTitle></CardHeader>
            <CardContent>
              <ul className="space-y-2 text-sm">
                {(Array.isArray(activity) ? activity : []).map((item, i) => (
                  <li key={item.id ?? i}>
                    <span className="font-medium">{item.sub}</span> {item.action}
                    {item.resource_type && <span className="text-muted-foreground"> · {item.resource_type}</span>}
                    <span className="text-muted-foreground ml-2">{item.occurred_at ? new Date(item.occurred_at).toLocaleString() : ''}</span>
                  </li>
                ))}
                {(!Array.isArray(activity) || !activity.length) && <li className="text-muted-foreground">No activity.</li>}
              </ul>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="settings">
          <Card>
            <CardHeader><CardTitle>Workspace settings</CardTitle></CardHeader>
            <CardContent><p className="text-muted-foreground">Rename, description, GA4GH dataset link. Owner only.</p></CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
