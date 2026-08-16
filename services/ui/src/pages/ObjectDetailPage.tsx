import { Link, useParams, useSearch } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiGet, apiPost } from '@/api/client';
import type { DrsObject } from '@/api/types';
import { attachMetadataRef } from '@/api/metadata';
import { ObjectLineageTab } from '@/components/ObjectLineageTab';
import { DataTypeIcon } from '@/components/DataTypeIcon';
import { Button } from '@/components/ui/button';
import { ArrowLeft, Download, Loader2, ExternalLink, Eye } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';
import { useEffect, useState } from 'react';
import { formatBytes } from '@/lib/utils';
import { errorMessageFromUnknown } from '@/lib/apiErrorReport';
import { ErrorWithReport } from '@/components/ErrorWithReport';
import {
  canStreamPreview,
  downloadWithAuth,
  drsStreamUrl,
  fetchPreviewText,
  PREVIEW_MAX_BYTES,
  wouldPreviewByType,
} from '@/lib/filePreview';
import { drsStorageKind } from '@/lib/drsStorage';
import { StartAnalysisDialog } from '@/components/StartAnalysisDialog';
import { NoopExecutorBanner } from '@/components/NoopExecutorBanner';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

type WorkspaceRow = { id: string; name: string };

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
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [linkWorkspaceId, setLinkWorkspaceId] = useState('');
  const [linkNotice, setLinkNotice] = useState<string | null>(null);
  const [metaAlias, setMetaAlias] = useState('');
  const qc = useQueryClient();

  const { data: obj, isLoading, error } = useQuery({
    queryKey: ['drs', 'object', id],
    queryFn: () => apiGet<DrsObject>(`/ga4gh/drs/v1/objects/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  const kind = obj ? drsStorageKind(obj) : 'unknown';
  const displayName = obj?.name ?? id;
  const isEncrypted = obj?.is_encrypted === true;
  const previewByType = obj ? wouldPreviewByType(displayName, obj.mime_type) : false;
  const streamPreviewable = obj
    ? canStreamPreview(kind, displayName, obj.mime_type)
    : false;

  const { data: workspaces } = useQuery({
    queryKey: ['workspaces'],
    queryFn: () => apiGet<WorkspaceRow[]>('/workspaces/v1/workspaces'),
    retry: false,
  });

  const linkToWorkspace = useMutation({
    mutationFn: (workspaceId: string) =>
      apiPost<{ linked: number }>(
        `/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}/objects/link`,
        { object_ids: [id] },
      ),
    onSuccess: (_data, workspaceId) => {
      void qc.invalidateQueries({ queryKey: ['drs', 'object', id] });
      const name = workspaces?.find((w) => w.id === workspaceId)?.name ?? workspaceId;
      setLinkNotice(t('object.linkWorkspaceSuccess', { name }));
    },
  });

  const attachMeta = useMutation({
    mutationFn: (alias: string | null) => attachMetadataRef(id, alias),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['drs', 'object', id] });
      setMetaAlias('');
    },
  });

  useEffect(() => {
    if (linkWorkspaceId || !workspaces?.length) return;
    setLinkWorkspaceId(workspaces[0].id);
  }, [workspaces, linkWorkspaceId]);

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
    setDownloadError(null);
    try {
      await downloadWithAuth(drsStreamUrl(id), obj?.name ?? id);
    } catch (e) {
      setDownloadError(errorMessageFromUnknown(e, t('object.downloadFailed')));
    } finally {
      setDownloading(false);
    }
  };

  const previewErrorMessage = previewError
    ? errorMessageFromUnknown(previewError, t('object.previewStreamFailed'))
    : null;

  if (!id) return <p className="text-muted-foreground">{t('object.noId')}</p>;
  if (isLoading) return <p className="text-muted-foreground">{t('object.loading')}</p>;
  if (error || !obj) {
    return (
      <div className="space-y-3 text-sm">
        <p className="text-destructive">{t('object.notFound')}</p>
        {search.analyze && (
          <p className="text-muted-foreground">{t('object.analyzeDeepLinkFailed')}</p>
        )}
      </div>
    );
  }

  const remote = externalUrl(obj);
  const workspaceId = obj.workspace_id ?? null;

  return (
    <div className="space-y-6">
      <NoopExecutorBanner compact />
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={'/data' as any}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <DataTypeIcon mimeType={obj.mime_type} className="text-primary" />
        <h1 className="text-2xl font-bold">{obj.name ?? obj.id}</h1>
        <div className="ml-auto flex flex-wrap gap-2">
          {workspaceId ? (
            <StartAnalysisDialog
              workspaceId={workspaceId}
              defaultDrsObjectId={id}
              autoOpen={search.analyze}
              initialStep={search.analyze ? 2 : undefined}
              triggerLabelKey="object.useInAnalysis"
            />
          ) : null}
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

      {downloadError && (
        <ErrorWithReport
          errorMessage={downloadError}
          context="drs-download"
          lastApi={{ method: 'GET', path: `/ga4gh/drs/v1/objects/${id}/stream` }}
        />
      )}

      {!workspaceId && (
        <Card className="border-amber-500/30 bg-amber-500/5">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t('object.linkWorkspaceFirst')}</CardTitle>
            <p className="text-sm text-muted-foreground font-normal">{t('object.linkWorkspaceHint')}</p>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 sm:flex-row sm:items-end">
            <div className="flex-1 space-y-2">
              <Select value={linkWorkspaceId} onValueChange={setLinkWorkspaceId}>
                <SelectTrigger>
                  <SelectValue placeholder={t('data.workspaceLabel')} />
                </SelectTrigger>
                <SelectContent>
                  {(workspaces ?? []).map((w) => (
                    <SelectItem key={w.id} value={w.id}>
                      {w.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              type="button"
              disabled={!linkWorkspaceId || linkToWorkspace.isPending}
              onClick={() => linkToWorkspace.mutate(linkWorkspaceId)}
            >
              {linkToWorkspace.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                t('object.linkWorkspaceAction')
              )}
            </Button>
          </CardContent>
          {linkNotice && <CardContent className="pt-0 text-sm text-emerald-700 dark:text-emerald-400">{linkNotice}</CardContent>}
        </Card>
      )}

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t('object.metaRef')}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="text-sm">
            {obj.metadata_ref ? (
              <Link
                to={'/metadata/submissions/$alias' as any}
                params={{ alias: obj.metadata_ref } as any}
                className="font-mono text-primary hover:underline"
              >
                {obj.metadata_ref}
              </Link>
            ) : (
              <span className="text-muted-foreground">{t('object.metaRefNone')}</span>
            )}
          </div>
          <label className="text-sm">
            <span className="sr-only">{t('object.metaRefAlias')}</span>
            <input
              className="border rounded px-2 py-1 bg-background font-mono text-sm"
              value={metaAlias}
              onChange={(e) => setMetaAlias(e.target.value)}
              placeholder={t('object.metaRefAlias')}
            />
          </label>
          <Button
            type="button"
            size="sm"
            disabled={!metaAlias.trim() || attachMeta.isPending}
            onClick={() => attachMeta.mutate(metaAlias.trim())}
          >
            {t('object.metaRefAttach')}
          </Button>
          {obj.metadata_ref && (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={attachMeta.isPending}
              onClick={() => attachMeta.mutate(null)}
            >
              {t('object.metaRefDetach')}
            </Button>
          )}
        </CardContent>
      </Card>

      {showPreview && streamPreviewable && (
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t('object.preview')}</CardTitle>
            {isEncrypted && (
              <p className="text-xs text-muted-foreground font-normal">{t('object.previewEncryptedNote')}</p>
            )}
            {obj.size != null && obj.size > PREVIEW_MAX_BYTES && (
              <p className="text-xs text-muted-foreground font-normal">{t('object.previewTooLarge')}</p>
            )}
          </CardHeader>
          <CardContent>
            {previewLoading ? (
              <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
            ) : previewErrorMessage ? (
              <ErrorWithReport
                errorMessage={previewErrorMessage}
                context="drs-preview"
                lastApi={{ method: 'GET', path: `/ga4gh/drs/v1/objects/${id}/stream` }}
              />
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

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t('object.summary')}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 sm:grid-cols-2 text-sm">
          <div>
            <p className="text-muted-foreground">{t('object.storageKind')}</p>
            <p className="font-medium">
              {kind === 'url' ? t('object.storageUrl') : t('object.storageManaged')}
              {isEncrypted ? ` · ${t('object.storageEncrypted')}` : ''}
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
          {obj.checksum_status && obj.checksum_status !== 'computed' && (
            <div className="sm:col-span-2">
              <p className="text-muted-foreground">{t('object.checksumStatus')}</p>
              <p
                className={
                  obj.checksum_status.startsWith('failed:')
                    ? 'text-sm text-destructive'
                    : 'text-sm text-amber-600 dark:text-amber-400'
                }
              >
                {obj.checksum_status.startsWith('failed:')
                  ? t('object.checksumFailed')
                  : obj.checksum_status === 'pending'
                    ? t('object.checksumPending')
                    : obj.checksum_status === 'deferred_low_power'
                      ? t('object.checksumDeferred')
                      : obj.checksum_status}
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
