/* eslint-disable @typescript-eslint/no-explicit-any */
import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { FolderPlus } from 'lucide-react';

interface Workspace {
  id: string;
  name: string;
  description: string | null;
  slug: string;
}

export function WorkspaceListPage() {
  const { data: workspaces, isLoading, error } = useQuery({
    queryKey: ['workspaces'],
    queryFn: () => apiGet<Workspace[]>('/workspaces/v1/workspaces'),
    retry: false,
  });

  if (isLoading) return <p className="text-muted-foreground">Loading workspaces…</p>;
  if (error) return <p className="text-destructive">Failed to load workspaces.</p>;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Workspaces</h1>
          <p className="text-muted-foreground">Shared project containers for data, workflows, and cohorts.</p>
        </div>
        <Button asChild>
          <Link to={"/workspaces/new" as any}>
            <FolderPlus className="mr-2 h-4 w-4" />
            New Workspace
          </Link>
        </Button>
      </div>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {workspaces?.map((ws) => (
          <Link key={ws.id} to={"/workspaces/" + ws.id}>
            <Card className="h-full transition-colors hover:bg-muted/50">
              <CardHeader className="pb-2">
                <CardTitle className="text-lg">{ws.name}</CardTitle>
                {ws.description && (
                  <p className="text-sm text-muted-foreground line-clamp-2">{ws.description}</p>
                )}
              </CardHeader>
              <CardContent>
                <span className="text-xs text-muted-foreground">{ws.slug}</span>
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>
      {workspaces?.length === 0 && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <p className="text-muted-foreground">No workspaces yet.</p>
            <Button asChild className="mt-4">
              <Link to={"/workspaces/new" as any}>Create your first workspace</Link>
            </Button>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
