import { useState } from 'react';
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
import { apiPost } from '@/api/client';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { useI18n } from '@/i18n/I18nProvider';
import { FolderPlus, Loader2 } from 'lucide-react';

interface IngestJobResponse {
  job_id: string;
  status: string;
  job_type: string;
  result?: { object_ids?: string[] };
}

export interface AddDataDialogProps {
  onSuccess?: (objectId: string) => void;
  defaultWorkspaceId?: string;
}

export function AddDataDialog({ onSuccess, defaultWorkspaceId }: AddDataDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const { data: config } = useAdminConfig();
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<'url' | 'location'>('url');
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [mime, setMime] = useState('');
  const [workspaceId, setWorkspaceId] = useState(defaultWorkspaceId ?? 'demo-workspace-01');
  const [backend, setBackend] = useState('');
  const [storageKey, setStorageKey] = useState('');
  const [size, setSize] = useState('');
  const [error, setError] = useState<string | null>(null);

  const defaultBackend = config?.storage?.backend ?? 'local';

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
              storage_backend: (backend.trim() || defaultBackend),
              storage_key: storageKey.trim(),
              size: Number.parseInt(size, 10) || 0,
              ...(name.trim() ? { name: name.trim() } : {}),
              ...(mime.trim() ? { mime_type: mime.trim() } : {}),
            };

      return apiPost<IngestJobResponse>('/api/v1/ingest/register', {
        client_request_id: `ferrum-ui-register-${crypto.randomUUID()}`,
        ...(workspaceId.trim() ? { workspace_id: workspaceId.trim() } : {}),
        items: [item],
      });
    },
    onSuccess: (data) => {
      const id = data.result?.object_ids?.[0];
      if (data.status === 'succeeded' && id) {
        void qc.invalidateQueries({ queryKey: ['drs', 'objects'] });
        setOpen(false);
        setError(null);
        onSuccess?.(id);
      } else if (data.status === 'failed') {
        setError(t('data.registerFailed'));
      } else {
        setError(t('data.registerJob', { jobId: data.job_id, status: data.status }));
      }
    },
    onError: (e: Error) => setError(e.message || t('data.registerFailed')),
  });

  const canSubmit =
    mode === 'url' ? url.trim().length > 0 : storageKey.trim().length > 0 && Number.parseInt(size, 10) > 0;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="default" className="gap-2">
          <FolderPlus className="h-4 w-4" />
          {t('data.add')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{t('data.addTitle')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('data.addDescription')}</p>
        </DialogHeader>
        <Tabs value={mode} onValueChange={(v) => setMode(v as 'url' | 'location')}>
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="url">{t('data.addByUrl')}</TabsTrigger>
            <TabsTrigger value="location">{t('data.addByLocation')}</TabsTrigger>
          </TabsList>
          <TabsContent value="url" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label htmlFor="add-url">{t('data.urlLabel')}</Label>
              <Input
                id="add-url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder={t('data.urlPlaceholder')}
              />
            </div>
            <p className="text-xs text-muted-foreground">{t('data.guideStep2')}</p>
          </TabsContent>
          <TabsContent value="location" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label htmlFor="add-backend">{t('data.backendLabel')}</Label>
              <Input
                id="add-backend"
                value={backend}
                onChange={(e) => setBackend(e.target.value)}
                placeholder={defaultBackend}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="add-key">{t('data.keyLabel')}</Label>
              <Input
                id="add-key"
                value={storageKey}
                onChange={(e) => setStorageKey(e.target.value)}
                placeholder={t('data.keyPlaceholder')}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="add-size">{t('data.sizeLabel')}</Label>
              <Input
                id="add-size"
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
        <div className="space-y-2">
          <Label htmlFor="add-ws">{t('data.workspaceLabel')}</Label>
          <Input
            id="add-ws"
            value={workspaceId}
            onChange={(e) => setWorkspaceId(e.target.value)}
            placeholder={t('data.workspacePlaceholder')}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="add-name">{t('data.nameLabel')} ({t('common.optional')})</Label>
          <Input id="add-name" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="space-y-2">
          <Label htmlFor="add-mime">{t('data.mimeLabel')} ({t('common.optional')})</Label>
          <Input id="add-mime" value={mime} onChange={(e) => setMime(e.target.value)} placeholder="application/octet-stream" />
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}
        <Button
          type="button"
          onClick={() => register.mutate()}
          disabled={!canSubmit || register.isPending}
          className="gap-2"
        >
          {register.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
          {t('data.register')}
        </Button>
        <p className="text-xs text-muted-foreground">{t('data.guideOptional')}</p>
      </DialogContent>
    </Dialog>
  );
}
