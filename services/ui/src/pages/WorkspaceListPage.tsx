/* eslint-disable @typescript-eslint/no-explicit-any */
import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { FolderPlus } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';

interface Workspace {
  id: string;
  name: string;
  description: string | null;
  slug: string;
}

export function WorkspaceListPage() {
  const { t } = useI18n();
  const { data: workspaces, isLoading, error } = useQuery({
    queryKey: ['workspaces'],
    queryFn: () => apiGet<Workspace[]>('/workspaces/v1/workspaces'),
    retry: false,
  });

  if (isLoading) return <p className="text-muted-foreground">{t('workspaceList.loading')}</p>;
  if (error) {
    const msg = error instanceof Error ? error.message : String(error);
    return (
      <div className="space-y-2">
        <p className="text-destructive font-medium">{t('workspaceList.failed')}</p>
        <p className="text-sm text-muted-foreground font-mono break-all">{t('common.error')}: {msg}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('workspaceList.title')}</h1>
          <p className="text-muted-foreground">{t('workspaceList.subtitle')}</p>
        </div>
        <Button asChild>
          <Link to={"/workspaces/new" as any}>
            <FolderPlus className="mr-2 h-4 w-4" />
            {t('workspaceList.new')}
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
            <p className="text-muted-foreground">{t('workspaceList.empty')}</p>
            <Button asChild className="mt-4">
              <Link to={"/workspaces/new" as any}>{t('workspaceList.createFirst')}</Link>
            </Button>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
