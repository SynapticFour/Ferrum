import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiPost, apiPostFormData } from '@/api/client';
import { useAdminConfig, isCrypt4ghIngestReady } from '@/hooks/useAdminConfig';
import { useIngestJobPoller } from '@/hooks/useIngestJobs';
import { useIngestJobsStore, type IngestJobKind } from '@/stores/ingestJobs';
import { useI18n } from '@/i18n/I18nProvider';
import { decodeJwtPayload } from '@/lib/auth';
import { ingestClientRequestId } from '@/lib/ingestClientRequestId';
import { useAuthStore } from '@/stores/auth';
import { Database, Loader2, Upload } from 'lucide-react';
import { cn } from '@/lib/utils';

interface IngestJobResponse {
  job_id: string;
  status: string;
  job_type: string;
  result?: { object_ids?: string[] };
}

export interface ImportToDrsDialogProps {
  onSuccess?: (objectId: string) => void;
  /** When set, imported objects are also linked to this workspace after ingest. */
  linkToWorkspaceId?: string;
  triggerVariant?: 'default' | 'outline';
  triggerLabelKey?: string;
}

async function linkObjectsToWorkspace(workspaceId: string, objectIds: string[]) {
  if (!objectIds.length) return;
  await apiPost<{ linked: number }>(
    `/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}/objects/link`,
    { object_ids: objectIds },
  );
}

export function ImportToDrsDialog({
  onSuccess,
  linkToWorkspaceId,
  triggerVariant = 'default',
  triggerLabelKey = 'data.importToDrs',
}: ImportToDrsDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const trackJob = useIngestJobsStore((s) => s.trackJob);
  const updateJob = useIngestJobsStore((s) => s.updateJob);
  const removeJob = useIngestJobsStore((s) => s.removeJob);
  const { data: config } = useAdminConfig();
  const fileRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<'upload' | 'url' | 'location'>('upload');
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [mime, setMime] = useState('');
  const [alsoLinkWorkspace, setAlsoLinkWorkspace] = useState(!!linkToWorkspaceId);
  const [backend, setBackend] = useState('');
  const [storageKey, setStorageKey] = useState('');
  const [size, setSize] = useState('');
  const [encryptUpload, setEncryptUpload] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [jobStatus, setJobStatus] = useState<string | null>(null);
  const activeJobIdRef = useRef<string | null>(null);
  activeJobIdRef.current = activeJobId;

  const defaultBackend = config?.storage?.backend ?? 'local';
  const crypt4ghIngestReady = isCrypt4ghIngestReady(config);
  const workspaceToLink = alsoLinkWorkspace ? linkToWorkspaceId : undefined;

  useEffect(() => {
    if (config && !crypt4ghIngestReady) {
      setEncryptUpload(false);
    }
  }, [config, crypt4ghIngestReady]);

  const finishImport = async (objectId: string) => {
    if (workspaceToLink) {
      await linkObjectsToWorkspace(workspaceToLink, [objectId]);
      void qc.invalidateQueries({ queryKey: ['drs', 'objects', 'workspace', workspaceToLink] });
      void qc.invalidateQueries({ queryKey: ['workspace', workspaceToLink, 'contents'] });
    }
    void qc.invalidateQueries({ queryKey: ['drs', 'objects'] });
    setOpen(false);
    setError(null);
    onSuccess?.(objectId);
  };

  const handleIngestJobDone = useCallback(
    async (status: string, result?: { object_ids?: string[] }) => {
      const jobId = activeJobIdRef.current;
      setActiveJobId(null);
      setJobStatus(status);
      if (jobId) {
        const objectId = result?.object_ids?.[0];
        updateJob(jobId, { status, objectId, finishedAt: Date.now() });
        if (status === 'succeeded' || status === 'failed') {
          window.setTimeout(() => removeJob(jobId), 8000);
        }
      }
      if (status === 'succeeded') {
        const id = result?.object_ids?.[0];
        if (id) {
          await finishImport(id);
        } else {
          setError(t('data.ingestJobNoObject'));
        }
      } else {
        setError(t('data.ingestJobFailed'));
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [workspaceToLink, linkToWorkspaceId, alsoLinkWorkspace, onSuccess, t, updateJob, removeJob],
  );

  useIngestJobPoller(activeJobId, handleIngestJobDone);

  const handleIngestResponse = async (
    data: IngestJobResponse,
    failedKey: string,
    jobKey: string,
    kind: IngestJobKind,
  ) => {
    const id = data.result?.object_ids?.[0];
    if (data.status === 'succeeded' && id) {
      await finishImport(id);
    } else if (data.status === 'failed') {
      setError(t(failedKey));
    } else if (data.job_id) {
      setActiveJobId(data.job_id);
      setJobStatus(data.status);
      setError(null);
      trackJob({
        jobId: data.job_id,
        status: data.status,
        kind,
      });
    } else {
      setError(t(jobKey, { jobId: data.job_id, status: data.status }));
    }
  };

  const register = useMutation({
    mutationFn: async () => {
      const item =
        mode === 'url'
          ? {
              kind: 'url' as const,
              url: url.trim(),
              ...(name.trim() ? { name: name.trim() } : {}),
              ...(mime.trim() ? { mime_type: mime.trim() } : {}),
            }
          : {
              kind: 'existing_object' as const,
              storage_backend: backend.trim() || defaultBackend,
              storage_key: storageKey.trim(),
              size: Number.parseInt(size, 10) || 0,
              ...(name.trim() ? { name: name.trim() } : {}),
              ...(mime.trim() ? { mime_type: mime.trim() } : {}),
            };

      return apiPost<IngestJobResponse>('/api/v1/ingest/register', {
        client_request_id: ingestClientRequestId('register', ingestSub),
        ...(workspaceToLink ? { workspace_id: workspaceToLink } : {}),
        items: [item],
      });
    },
    onSuccess: async (data) => {
      await handleIngestResponse(data, 'data.registerFailed', 'data.registerJob', 'register');
    },
    onError: (e: Error) => setError(e.message || t('data.registerFailed')),
  });

  const upload = useMutation({
    mutationFn: async (file: File) => {
      const fd = new FormData();
      fd.append('file', file);
      fd.append('client_request_id', ingestClientRequestId('upload', ingestSub));
      if (encryptUpload) fd.append('encrypt', 'true');
      if (workspaceToLink) fd.append('workspace_id', workspaceToLink);
      return apiPostFormData<IngestJobResponse>('/api/v1/ingest/upload', fd);
    },
    onSuccess: async (data) => {
      await handleIngestResponse(data, 'data.uploadFailed', 'data.uploadJob', 'upload');
    },
    onError: (e: Error) => setError(e.message || t('data.uploadFailed')),
  });

  const registerCanSubmit =
    mode === 'url' ? url.trim().length > 0 : storageKey.trim().length > 0 && Number.parseInt(size, 10) > 0;
  const pending = register.isPending || upload.isPending || !!activeJobId;
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const ingestSub = useMemo(() => {
    if (!passportJwt) return null;
    const claims = decodeJwtPayload(passportJwt);
    return typeof claims?.sub === 'string' ? claims.sub : null;
  }, [passportJwt]);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant={triggerVariant} className="gap-2" data-testid="import-to-drs-trigger">
          {mode === 'upload' && triggerVariant === 'outline' ? (
            <Upload className="h-4 w-4" />
          ) : (
            <Database className="h-4 w-4" />
          )}
          {t(triggerLabelKey)}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('data.importTitle')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('data.importDescription')}</p>
        </DialogHeader>

        <Tabs value={mode} onValueChange={(v) => setMode(v as typeof mode)}>
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="upload">{t('data.addByUpload')}</TabsTrigger>
            <TabsTrigger value="url">{t('data.addByUrl')}</TabsTrigger>
            <TabsTrigger value="location">{t('data.addByLocation')}</TabsTrigger>
          </TabsList>
          <TabsContent value="upload" className="space-y-3 pt-2">
            <p className="text-xs text-muted-foreground">{t('data.uploadTabHint')}</p>
            <input
              ref={fileRef}
              type="file"
              className="hidden"
              onChange={(ev) => {
                const file = ev.target.files?.[0];
                ev.target.value = '';
                if (file) upload.mutate(file);
              }}
            />
            <Button
              type="button"
              variant="outline"
              className="gap-2 w-full"
              disabled={pending}
              onClick={() => fileRef.current?.click()}
            >
              {upload.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
              {t('data.upload')}
            </Button>
            <label
              className={cn(
                'flex items-center gap-2 text-sm',
                !crypt4ghIngestReady ? 'cursor-not-allowed opacity-50' : 'cursor-pointer text-muted-foreground',
              )}
            >
              <input
                type="checkbox"
                checked={encryptUpload}
                disabled={!crypt4ghIngestReady || pending}
                onChange={(e) => setEncryptUpload(e.target.checked)}
                className="rounded border-border disabled:cursor-not-allowed"
              />
              {t('data.encryptDefault')}
            </label>
            {!crypt4ghIngestReady && (
              <p className="text-xs text-muted-foreground">{t('data.encryptPilotUnavailable')}</p>
            )}
          </TabsContent>
          <TabsContent value="url" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label htmlFor="import-url">{t('data.urlLabel')}</Label>
              <Input
                id="import-url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder={t('data.urlPlaceholder')}
              />
            </div>
            <p className="text-xs text-muted-foreground">{t('data.guideStep2')}</p>
          </TabsContent>
          <TabsContent value="location" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label htmlFor="import-backend">{t('data.backendLabel')}</Label>
              <Input
                id="import-backend"
                value={backend}
                onChange={(e) => setBackend(e.target.value)}
                placeholder={defaultBackend}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="import-key">{t('data.keyLabel')}</Label>
              <Input
                id="import-key"
                value={storageKey}
                onChange={(e) => setStorageKey(e.target.value)}
                placeholder={t('data.keyPlaceholder')}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="import-size">{t('data.sizeLabel')}</Label>
              <Input
                id="import-size"
                type="number"
                min={1}
                value={size}
                onChange={(e) => setSize(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">{t('data.sizeHint')}</p>
            </div>
            <p className="text-xs text-muted-foreground">{t('data.guideStep3')}</p>
          </TabsContent>
        </Tabs>

        {mode !== 'upload' && (
          <>
            <div className="space-y-2">
              <Label htmlFor="import-name">
                {t('data.nameLabel')} ({t('common.optional')})
              </Label>
              <Input id="import-name" value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="import-mime">
                {t('data.mimeLabel')} ({t('common.optional')})
              </Label>
              <Input
                id="import-mime"
                value={mime}
                onChange={(e) => setMime(e.target.value)}
                placeholder={t('data.mimePlaceholder')}
              />
            </div>
          </>
        )}

        {linkToWorkspaceId && (
          <label className="flex cursor-pointer items-start gap-2 text-sm">
            <input
              type="checkbox"
              checked={alsoLinkWorkspace}
              onChange={(e) => setAlsoLinkWorkspace(e.target.checked)}
              className="mt-1 rounded border-border"
            />
            <span>
              <span className="font-medium">{t('data.alsoLinkWorkspace')}</span>
              <span className="block text-xs text-muted-foreground">{t('data.alsoLinkWorkspaceHint')}</span>
            </span>
          </label>
        )}

        {activeJobId && (
          <p className="text-sm text-muted-foreground flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t('data.ingestJobRunning', { jobId: activeJobId, status: jobStatus ?? 'queued' })}
          </p>
        )}

        {error && <p className="text-sm text-destructive">{error}</p>}

        {mode !== 'upload' && (
          <Button
            type="button"
            onClick={() => register.mutate()}
            disabled={!registerCanSubmit || pending}
            className="gap-2"
          >
            {register.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {t('data.register')}
          </Button>
        )}
      </DialogContent>
    </Dialog>
  );
}
