import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiGet } from '@/api/client';
import { ImportToDrsDialog } from '@/components/ImportToDrsDialog';
import { DatasetCatalogPanel } from '@/components/DatasetCatalogPanel';
import { IngestJobsBanner } from '@/components/IngestJobsBanner';
import { OntIngestDialog } from '@/components/OntIngestDialog';
import { useI18n } from '@/i18n/I18nProvider';
import { formatBytes } from '@/lib/utils';
import { PublishDatasetDialog } from '@/components/PublishDatasetDialog';
import { Database, AlertCircle, Globe, Filter } from 'lucide-react';

interface DrsObject {
  id: string;
  name?: string;
  description?: string;
  size?: number;
  mime_type?: string;
  created_time?: string;
}

type WorkspaceRow = { id: string; name: string };

const ALL_WORKSPACES = '__all__';

export function DataBrowser() {
  const { t } = useI18n();
  const [tab, setTab] = useState('mine');
  const [workspaceFilter, setWorkspaceFilter] = useState(ALL_WORKSPACES);
  const [uploadBanner, setUploadBanner] = useState<{
    kind: 'success' | 'error';
    text: string;
    objectId?: string;
  } | null>(null);

  const { data: workspaces } = useQuery({
    queryKey: ['workspaces'],
    queryFn: () => apiGet<WorkspaceRow[]>('/workspaces/v1/workspaces'),
    retry: false,
  });

  const objectsUrl =
    workspaceFilter === ALL_WORKSPACES
      ? '/ga4gh/drs/v1/objects'
      : `/ga4gh/drs/v1/objects?workspace_id=${encodeURIComponent(workspaceFilter)}&limit=500`;

  const { data: objects, isLoading, error } = useQuery({
    queryKey: ['drs', 'objects', workspaceFilter],
    queryFn: () => apiGet<DrsObject[]>(objectsUrl),
    retry: false,
  });

  const list = Array.isArray(objects) ? objects : [];
  const workspaceLabel =
    workspaceFilter === ALL_WORKSPACES
      ? t('data.workspaceAll')
      : workspaces?.find((w) => w.id === workspaceFilter)?.name ?? workspaceFilter;

  return (
    <div className="space-y-6">
      <Card className="border-primary/20 bg-primary/5">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t('data.importVsLinkTitle')}</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-2">
          <p>{t('data.importVsLinkBody')}</p>
          <p className="text-xs pt-1">1. {t('data.guideStep1')}</p>
          <p className="text-xs">2. {t('data.guideStep2')}</p>
          <p className="text-xs">3. {t('data.guideStep3')}</p>
        </CardContent>
      </Card>

      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('data.title')}</h1>
          <p className="text-muted-foreground">{t('data.subtitle')}</p>
        </div>
        <div className="flex flex-col items-stretch gap-2 sm:items-end">
          <div className="flex flex-wrap items-center gap-2">
            <ImportToDrsDialog
              onSuccess={(id) => {
                setUploadBanner({
                  kind: 'success',
                  text: t('data.registerSuccess', { id }),
                  objectId: id,
                });
              }}
            />
            <OntIngestDialog />
          </div>
        </div>
      </div>

      {uploadBanner && (
        <div
          className={
            uploadBanner.kind === 'success'
              ? 'flex flex-wrap items-center gap-2 rounded-md border border-emerald-500/40 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-400'
              : 'flex items-center gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive'
          }
        >
          <span>{uploadBanner.text}</span>
          {uploadBanner.kind === 'success' && uploadBanner.objectId && (
            <Link
              to={`/data/objects/${uploadBanner.objectId}` as any}
              className="font-medium underline"
            >
              {t('data.openObject')}
            </Link>
          )}
          <button
            type="button"
            className="ms-auto text-xs underline opacity-70"
            onClick={() => setUploadBanner(null)}
          >
            {t('common.dismiss')}
          </button>
        </div>
      )}

      <IngestJobsBanner
        onSucceeded={(objectId) => {
          setUploadBanner({
            kind: 'success',
            text: t('data.registerSuccess', { id: objectId }),
            objectId,
          });
        }}
      />

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="mine" className="gap-1">
            <Database className="h-3.5 w-3.5" />
            {t('data.tabMine')}
          </TabsTrigger>
          <TabsTrigger value="catalog" className="gap-1" data-testid="data-catalog-tab">
            <Globe className="h-3.5 w-3.5" />
            {t('data.tabCatalog')}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="mine" className="mt-4 space-y-4">
          {error && (
            <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
              <AlertCircle className="h-4 w-4 shrink-0" />
              {t('data.listUnavailable')}
            </div>
          )}

          <Card>
            <CardHeader>
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <Database className="h-4 w-4" />
                    {t('data.objects')}
                  </CardTitle>
                  <p className="text-sm text-muted-foreground mt-1">{t('data.objectsHint')}</p>
                </div>
                {(workspaces?.length ?? 0) > 0 && (
                  <label className="flex items-center gap-2 text-sm shrink-0">
                    <Filter className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-muted-foreground">{t('data.workspaceFilter')}</span>
                    <select
                      className="rounded-md border border-input bg-background px-2 py-1 text-sm"
                      value={workspaceFilter}
                      onChange={(e) => setWorkspaceFilter(e.target.value)}
                      aria-label={t('data.workspaceFilter')}
                    >
                      <option value={ALL_WORKSPACES}>{t('data.workspaceAll')}</option>
                      {workspaces!.map((ws) => (
                        <option key={ws.id} value={ws.id}>
                          {ws.name}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
              {workspaceFilter !== ALL_WORKSPACES && (
                <p className="text-xs text-muted-foreground">
                  {t('data.workspaceFilterActive', { name: workspaceLabel })}
                </p>
              )}
            </CardHeader>
            <CardContent>
              {isLoading && <p className="text-muted-foreground text-sm">{t('common.loading')}</p>}
              {!isLoading && list.length === 0 && !error && (
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>{t('data.noObjects')}</p>
                  {workspaceFilter !== ALL_WORKSPACES && (
                    <p className="rounded-md border bg-muted/30 p-3 text-xs leading-relaxed">
                      {t('data.workspaceEmptySeedHint')}
                    </p>
                  )}
                </div>
              )}
              {!isLoading && list.length > 0 && (
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-border">
                        <th className="py-2 text-start font-medium">{t('data.colId')}</th>
                        <th className="py-2 text-start font-medium">{t('data.colName')}</th>
                        <th className="py-2 text-start font-medium">{t('data.colSize')}</th>
                        <th className="py-2 text-start font-medium">{t('data.colType')}</th>
                        <th className="py-2 text-start font-medium">{t('data.colActions')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {list.map((obj) => (
                        <tr key={obj.id} className="border-b border-border/50">
                          <td className="py-2 font-mono text-xs">{obj.id}</td>
                          <td className="py-2">{obj.name ?? '—'}</td>
                          <td className="py-2">
                            {obj.size != null && obj.size > 0 ? formatBytes(obj.size) : '—'}
                          </td>
                          <td className="py-2">{obj.mime_type ?? '—'}</td>
                          <td className="py-2 flex flex-wrap gap-2">
                            <Link to={`/data/objects/${obj.id}` as any} className="text-primary hover:underline">
                              {t('common.view')}
                            </Link>
                            <Link
                              to={`/data/objects/${obj.id}?analyze=1` as any}
                              className="text-primary hover:underline"
                            >
                              {t('object.useInAnalysis')}
                            </Link>
                            <PublishDatasetDialog
                              objectId={obj.id}
                              defaultName={obj.name}
                              onPublished={(adsId) => {
                                setUploadBanner({
                                  kind: 'success',
                                  text: t('data.publishSuccess', { id: adsId }),
                                });
                              }}
                            />
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

        <TabsContent value="catalog" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Globe className="h-4 w-4" />
                {t('data.catalogTitle')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <DatasetCatalogPanel compact />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
