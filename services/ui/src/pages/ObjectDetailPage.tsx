import { Link, useParams, useSearch } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import type { DrsObject } from '@/api/types';
import { ObjectLineageTab } from '@/components/ObjectLineageTab';
import { DataTypeIcon } from '@/components/DataTypeIcon';
import { Button } from '@/components/ui/button';
import { ArrowLeft, Download, Loader2, ExternalLink, Eye } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';
import { useState } from 'react';
import { formatBytes } from '@/lib/utils';
import {
  canStreamPreview,
  downloadWithAuth,
  drsStreamUrl,
  fetchPreviewText,
  wouldPreviewByType,
} from '@/lib/filePreview';
import { StartAnalysisDialog } from '@/components/StartAnalysisDialog';

function storageKind(obj: DrsObject): 'managed' | 'url' | 'unknown' {
  const backend = (obj as DrsObject & { storage_backend?: string }).storage_backend;
  if (backend === 'url') return 'url';
  if (backend === 's3' || backend === 'local') return 'managed';
  const am = obj.access_methods?.[0];
  if (am?.type === 'https' && am.access_url?.url?.startsWith('http')) return 'url';
  return 'managed';
}

function externalUrl(obj: DrsObject): string | null {
  const am = obj.access_methods?.find((m) => m.type === 'https');
  const url = am?.access_url?.url;
  if (url?.startsWith('http')) return url;
  return null;
}

export function ObjectDetailPage() {
  const { t } = useI18n();
  const params = useParams({ strict: false }) as { objectId?: string };
  const search = useSearch({ strict: false }) as { analyze?: boolean };
  const id = params.objectId ?? '';
  const [downloading, setDownloading] = useState(false);
  const [showPreview, setShowPreview] = useState(false);

  const { data: obj, isLoading, error } = useQuery({
    queryKey: ['drs', 'object', id],
    queryFn: () => apiGet<DrsObject>(`/ga4gh/drs/v1/objects/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  const kind = obj ? storageKind(obj) : 'unknown';
  const displayName = obj?.name ?? id;
  const previewByType = obj ? wouldPreviewByType(displayName, obj.mime_type, obj.size) : false;
  const streamPreviewable = obj
    ? canStreamPreview(kind, displayName, obj.mime_type, obj.size)
    : false;

  const {
    data: previewText,
    isLoading: previewLoading,
    error: previewError,
  } = useQuery({
    queryKey: ['drs', 'object', id, 'preview'],
    enabled: showPreview && streamPreviewable && !!id,
    queryFn: () => fetchPreviewText(drsStreamUrl(id, true), t('object.previewTruncated')),
  });

  const handleDownload = async () => {
    setDownloading(true);
    try {
      await downloadWithAuth(drsStreamUrl(id), obj?.name ?? id);
    } finally {
      setDownloading(false);
    }
  };

  if (!id) return <p className="text-muted-foreground">{t('object.noId')}</p>;
  if (isLoading) return <p className="text-muted-foreground">{t('object.loading')}</p>;
  if (error || !obj) return <p className="text-destructive">{t('object.notFound')}</p>;

  const remote = externalUrl(obj);
  const workspaceId = (obj as DrsObject & { workspace_id?: string }).workspace_id ?? 'demo-workspace-01';

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={'/data' as any}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <DataTypeIcon mimeType={obj.mime_type} className="text-primary" />
        <h1 className="text-2xl font-bold">{obj.name ?? obj.id}</h1>
        <div className="ml-auto flex flex-wrap gap-2">
          <StartAnalysisDialog
            workspaceId={workspaceId}
            defaultDrsObjectId={id}
            autoOpen={search.analyze}
            initialStep={search.analyze ? 2 : undefined}
            triggerLabelKey="object.useInAnalysis"
          />
          {streamPreviewable && (
            <Button
              variant="outline"
              size="sm"
              className="gap-2"
              onClick={() => setShowPreview((v) => !v)}
            >
              <Eye className="h-4 w-4" />
              {t('object.preview')}
            </Button>
          )}
          <Button
            variant="default"
            size="sm"
            className="gap-2"
            onClick={() => void handleDownload()}
            disabled={downloading}
          >
            {downloading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            {t('object.download')}
          </Button>
        </div>
      </div>

      {showPreview && streamPreviewable && (
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t('object.preview')}</CardTitle>
          </CardHeader>
          <CardContent>
            {previewLoading ? (
              <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
            ) : previewError ? (
              <p className="text-sm text-destructive">{t('object.previewStreamFailed')}</p>
            ) : (
              <pre className="max-h-96 overflow-auto text-xs whitespace-pre-wrap break-all bg-muted/30 rounded p-3">
                {previewText ?? ''}
              </pre>
            )}
          </CardContent>
        </Card>
      )}

      {kind === 'url' && previewByType && (
        <p className="text-xs text-muted-foreground rounded-md border border-border bg-muted/20 px-3 py-2">
          {t('object.previewUrlBacked')}
        </p>
      )}

      {!streamPreviewable && previewByType && obj.size != null && obj.size > 256_000 && (
        <p className="text-xs text-muted-foreground">{t('object.previewTooLarge')}</p>
      )}

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t('object.summary')}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 sm:grid-cols-2 text-sm">
          <div>
            <p className="text-muted-foreground">{t('object.storageKind')}</p>
            <p className="font-medium">
              {kind === 'url' ? t('object.storageUrl') : t('object.storageManaged')}
            </p>
          </div>
          <div>
            <p className="text-muted-foreground">{t('data.colSize')}</p>
            <p className="font-medium">
              {obj.size != null && obj.size > 0 ? formatBytes(obj.size) : t('object.sizeUnknown')}
            </p>
          </div>
          <div>
            <p className="text-muted-foreground">{t('data.colType')}</p>
            <p className="font-medium">{obj.mime_type ?? '—'}</p>
          </div>
          {obj.checksums?.[0] && (
            <div>
              <p className="text-muted-foreground">{t('object.checksum')}</p>
              <p className="font-mono text-xs break-all">
                {obj.checksums[0].type}: {obj.checksums[0].checksum.slice(0, 16)}…
              </p>
            </div>
          )}
          {obj.description && (
            <div className="sm:col-span-2">
              <p className="text-muted-foreground">{t('object.description')}</p>
              <p>{obj.description}</p>
            </div>
          )}
          {remote && (
            <div className="sm:col-span-2">
              <p className="text-muted-foreground">{t('object.sourceUrl')}</p>
              <a
                href={remote}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 text-primary text-xs break-all hover:underline"
              >
                {remote}
                <ExternalLink className="h-3 w-3 shrink-0" />
              </a>
            </div>
          )}
          {obj.ont_metrics && Object.keys(obj.ont_metrics).length > 0 && (
            <div className="sm:col-span-2">
              <p className="text-muted-foreground">{t('object.ontMetrics')}</p>
              <pre className="text-xs bg-muted rounded p-2 overflow-auto max-h-32">
                {JSON.stringify(obj.ont_metrics, null, 2)}
              </pre>
            </div>
          )}
          {obj.gisaid_metadata && Object.keys(obj.gisaid_metadata).length > 0 && (
            <div className="sm:col-span-2">
              <p className="text-muted-foreground">{t('object.gisaidMetadata')}</p>
              <pre className="text-xs bg-muted rounded p-2 overflow-auto max-h-32">
                {JSON.stringify(obj.gisaid_metadata, null, 2)}
              </pre>
            </div>
          )}
        </CardContent>
      </Card>

      <Tabs defaultValue="details">
        <TabsList>
          <TabsTrigger value="details">{t('object.details')}</TabsTrigger>
          <TabsTrigger value="lineage">{t('object.lineage')}</TabsTrigger>
        </TabsList>
        <TabsContent value="details">
          <Card>
            <CardHeader>
              <CardTitle>{t('object.metadata')}</CardTitle>
            </CardHeader>
            <CardContent className="text-sm space-y-1">
              <p>
                <span className="text-muted-foreground">ID:</span>{' '}
                <code className="break-all">{obj.id}</code>
              </p>
              {obj.aliases && obj.aliases.length > 0 && (
                <p>
                  <span className="text-muted-foreground">{t('object.aliases')}:</span>{' '}
                  {obj.aliases.join(', ')}
                </p>
              )}
              {obj.created_time && (
                <p>
                  <span className="text-muted-foreground">{t('object.created')}:</span>{' '}
                  {new Date(obj.created_time).toLocaleString()}
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="lineage">
          <ObjectLineageTab objectId={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
