import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { listSubmissions } from '@/api/metadata';
import { useI18n } from '@/i18n/I18nProvider';
import { ApiAuthError } from '@/api/client';

export function MetadataSubmissionsPage() {
  const { t } = useI18n();
  const { data, isLoading, error } = useQuery({
    queryKey: ['metadata', 'submissions'],
    queryFn: () => listSubmissions(50),
    retry: false,
  });

  if (isLoading) return <div className="text-muted-foreground">{t('metadataList.loading')}</div>;
  if (error) {
    const status = error instanceof ApiAuthError ? error.status : 0;
    const body =
      status === 501 ? t('metadataList.disabled') : `${t('metadataList.failed')}: ${String(error)}`;
    return <div className="text-destructive">{body}</div>;
  }

  const items = data?.items ?? [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t('metadataList.title')}</h1>
        <p className="text-muted-foreground">{t('metadataList.subtitle')}</p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>{t('metadataList.cardTitle')}</CardTitle>
        </CardHeader>
        <CardContent>
          {items.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('metadataList.empty')}</p>
          ) : (
            <div className="divide-y">
              {items.map((row) => (
                <Link
                  key={row.alias}
                  to={'/metadata/submissions/$alias' as any}
                  params={{ alias: row.alias } as any}
                  className="flex flex-wrap items-center justify-between gap-2 py-3 hover:bg-muted/40 -mx-2 px-2 rounded"
                >
                  <div>
                    <p className="font-medium font-mono text-sm">{row.alias}</p>
                    <p className="text-xs text-muted-foreground">
                      {t('metadataList.updated')}: {row.updated_time ?? row.created_time}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary">{row.profile}</Badge>
                    <Badge variant="outline">v{row.version}</Badge>
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
