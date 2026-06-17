import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useCallback, useRef, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { apiGet, apiPostFormData } from '@/api/client';
import { AddDataDialog } from '@/components/AddDataDialog';
import { OntIngestDialog } from '@/components/OntIngestDialog';
import { useI18n } from '@/i18n/I18nProvider';
import { formatBytes } from '@/lib/utils';
import { useIngestJobPoller } from '@/hooks/useIngestJobs';
import { PublishDatasetDialog } from '@/components/PublishDatasetDialog';
import { Database, Upload, AlertCircle, Loader2 } from 'lucide-react';

interface DrsObject {
  id: string;
  name?: string;
  description?: string;
  size?: number;
  mime_type?: string;
  created_time?: string;
}

interface IngestJobResponse {
  job_id: string;
  status: string;
  job_type: string;
  result?: { object_ids?: string[]; self_uris?: string[]; size?: number };
  error?: unknown;
}

interface PendingJob {
  jobId: string;
  label: string;
  status: 'running' | 'succeeded' | 'failed';
  objectId?: string;
}

export function DataBrowser() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [encryptUpload, setEncryptUpload] = useState(true);
  const [uploadBanner, setUploadBanner] = useState<{
    kind: 'success' | 'error';
    text: string;
    objectId?: string;
  } | null>(null);
  const [pendingJobs, setPendingJobs] = useState<PendingJob[]>([]);
  const [pollJobId, setPollJobId] = useState<string | null>(null);

  const { data: objects, isLoading, error } = useQuery({
    queryKey: ['drs', 'objects'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    retry: false,
  });

  const onJobDone = useCallback(
    (status: string, result?: { object_ids?: string[] }) => {
      const id = result?.object_ids?.[0];
      setPendingJobs((prev) =>
        prev.map((j) =>
          j.jobId === pollJobId
            ? { ...j, status: status === 'succeeded' ? 'succeeded' : 'failed', objectId: id }
            : j,
        ),
      );
      if (status === 'succeeded' && id) {
        setUploadBanner({
          kind: 'success',
          text: t('data.uploadSuccess', { id }),
          objectId: id,
        });
        void queryClient.invalidateQueries({ queryKey: ['drs', 'objects'] });
      }
      setPollJobId(null);
    },
    [pollJobId, queryClient, t],
  );

  useIngestJobPoller(pollJobId, onJobDone);

  const uploadMutation = useMutation({
    mutationFn: async (files: File[]) => {
      const results: IngestJobResponse[] = [];
      for (const file of files) {
        const fd = new FormData();
        fd.append('file', file);
        fd.append('client_request_id', `ferrum-ui-${crypto.randomUUID()}`);
        if (encryptUpload) fd.append('encrypt', 'true');
        const res = await apiPostFormData<IngestJobResponse>('/api/v1/ingest/upload', fd);
        results.push(res);
      }
      return results;
    },
    onSuccess: (data) => {
      const jobs: PendingJob[] = [];
      for (const item of data) {
        const immediateId = item.result?.object_ids?.[0];
        if (item.status === 'succeeded' && immediateId) {
          setUploadBanner({
            kind: 'success',
            text: t('data.uploadSuccess', { id: immediateId }),
            objectId: immediateId,
          });
        } else if (item.status !== 'succeeded') {
          jobs.push({
            jobId: item.job_id,
            label: item.job_id.slice(0, 8),
            status: 'running',
          });
          setPollJobId(item.job_id);
        }
      }
      if (jobs.length) setPendingJobs((prev) => [...jobs, ...prev]);
      void queryClient.invalidateQueries({ queryKey: ['drs', 'objects'] });
    },
    onError: (e: Error) => {
      setUploadBanner({ kind: 'error', text: e.message || t('data.uploadFailed') });
    },
  });

  const list = Array.isArray(objects) ? objects : [];

  return (
    <div className="space-y-6">
      <Card className="border-primary/20 bg-primary/5">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t('data.guideTitle')}</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-1">
          <p>1. {t('data.guideStep1')}</p>
          <p>2. {t('data.guideStep2')}</p>
          <p>3. {t('data.guideStep3')}</p>
          <p className="text-xs pt-1">{t('data.guideOptional')}</p>
        </CardContent>
      </Card>

      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('data.title')}</h1>
          <p className="text-muted-foreground">{t('data.subtitle')}</p>
        </div>
        <div className="flex flex-col items-stretch gap-2 sm:items-end">
          <input
            ref={fileInputRef}
            type="file"
            multiple
            className="hidden"
            onChange={(ev) => {
              const files = ev.target.files ? Array.from(ev.target.files) : [];
              ev.target.value = '';
              if (files.length) uploadMutation.mutate(files);
            }}
          />
          <div className="flex flex-wrap items-center gap-2">
            <AddDataDialog
              onSuccess={(id) => {
                setUploadBanner({
                  kind: 'success',
                  text: t('data.registerSuccess', { id }),
                  objectId: id,
                });
              }}
            />
            <OntIngestDialog />
            <Button
              variant="outline"
              className="gap-2"
              disabled={uploadMutation.isPending}
              onClick={() => fileInputRef.current?.click()}
            >
              {uploadMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Upload className="h-4 w-4" />
              )}
              {t('data.upload')}
            </Button>
          </div>
          <label className="flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              checked={encryptUpload}
              onChange={(e) => setEncryptUpload(e.target.checked)}
              className="rounded border-border"
            />
            {t('data.encryptDefault')}
          </label>
          <p className="max-w-md text-xs text-muted-foreground">{t('data.encryptHint')}</p>
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
            className="ml-auto text-xs underline opacity-70"
            onClick={() => setUploadBanner(null)}
          >
            {t('common.dismiss')}
          </button>
        </div>
      )}

      {pendingJobs.length > 0 && (
        <Card>
          <CardHeader className="py-3">
            <CardTitle className="text-sm">{t('data.jobsTitle')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 pb-4">
            {pendingJobs.map((j) => (
              <div key={j.jobId} className="flex items-center gap-2 text-sm">
                {j.status === 'running' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
                <span className="font-mono text-xs">{j.label}</span>
                <span className="text-muted-foreground">
                  {j.status === 'running'
                    ? t('data.jobRunning')
                    : j.status === 'succeeded'
                      ? t('data.jobSucceeded')
                      : t('data.jobFailed')}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {t('data.listUnavailable')}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            {t('data.objects')}
          </CardTitle>
          <p className="text-sm text-muted-foreground">{t('data.objectsHint')}</p>
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">{t('common.loading')}</p>}
          {!isLoading && list.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">{t('data.noObjects')}</p>
          )}
          {!isLoading && list.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border">
                    <th className="py-2 text-left font-medium">{t('data.colId')}</th>
                    <th className="py-2 text-left font-medium">{t('data.colName')}</th>
                    <th className="py-2 text-left font-medium">{t('data.colSize')}</th>
                    <th className="py-2 text-left font-medium">{t('data.colType')}</th>
                    <th className="py-2 text-left font-medium">{t('data.colActions')}</th>
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
    </div>
  );
}
