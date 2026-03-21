import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useRef, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { apiGet, apiPostFormData } from '@/api/client';
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

export function DataBrowser() {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [encryptUpload, setEncryptUpload] = useState(false);
  const [uploadBanner, setUploadBanner] = useState<{ kind: 'success' | 'error'; text: string; objectId?: string } | null>(
    null,
  );

  const { data: objects, isLoading, error } = useQuery({
    queryKey: ['drs', 'objects'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    retry: false,
  });

  const uploadMutation = useMutation({
    mutationFn: async (file: File) => {
      const fd = new FormData();
      fd.append('file', file);
      fd.append('client_request_id', `ferrum-ui-${crypto.randomUUID()}`);
      if (encryptUpload) fd.append('encrypt', 'true');
      return apiPostFormData<IngestJobResponse>('/api/v1/ingest/upload', fd);
    },
    onSuccess: (data) => {
      const id = data.result?.object_ids?.[0];
      if (data.status === 'succeeded' && id) {
        setUploadBanner({
          kind: 'success',
          text: `Uploaded as DRS object ${id}.`,
          objectId: id,
        });
        void queryClient.invalidateQueries({ queryKey: ['drs', 'objects'] });
      } else {
        setUploadBanner({
          kind: 'success',
          text: `Job ${data.job_id}: ${data.status}`,
        });
        void queryClient.invalidateQueries({ queryKey: ['drs', 'objects'] });
      }
    },
    onError: (e: Error) => {
      setUploadBanner({ kind: 'error', text: e.message || 'Upload failed' });
    },
  });

  const list = Array.isArray(objects) ? objects : [];

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Data Browser</h1>
          <p className="text-muted-foreground">Browse and manage DRS objects.</p>
        </div>
        <div className="flex flex-col items-stretch gap-2 sm:items-end">
          <input
            ref={fileInputRef}
            type="file"
            className="hidden"
            onChange={(ev) => {
              const f = ev.target.files?.[0];
              ev.target.value = '';
              if (f) uploadMutation.mutate(f);
            }}
          />
          <div className="flex flex-wrap items-center gap-3">
            <label className="flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
              <input
                type="checkbox"
                checked={encryptUpload}
                onChange={(e) => setEncryptUpload(e.target.checked)}
                className="rounded border-border"
              />
              Crypt4GH (server key)
            </label>
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
              Upload file
            </Button>
          </div>
          <p className="max-w-md text-xs text-muted-foreground">
            Uses <code className="rounded bg-muted px-1">POST /api/v1/ingest/upload</code> (same API as Ferrum Lab Kit). Requires gateway with DRS + object storage.
          </p>
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
            <Link to={`/data/objects/${uploadBanner.objectId}` as any} className="font-medium underline">
              Open object
            </Link>
          )}
          <button
            type="button"
            className="ml-auto text-xs underline opacity-70"
            onClick={() => setUploadBanner(null)}
          >
            Dismiss
          </button>
        </div>
      )}
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          DRS list is not configured or unavailable. Upload may still work if ingest and storage are enabled on the gateway.
        </div>
      )}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            Objects
          </CardTitle>
          <p className="text-sm text-muted-foreground">
            Upload files from this page, or use{' '}
            <code className="rounded bg-muted px-1">/api/v1/ingest/*</code> /{' '}
            <code className="rounded bg-muted px-1">/ga4gh/drs/v1/ingest/*</code> — see{' '}
            <a
              href="https://github.com/SynapticFour/Ferrum/blob/main/docs/INGEST-LAB-KIT.md"
              className="text-primary underline"
              target="_blank"
              rel="noreferrer"
            >
              INGEST-LAB-KIT.md
            </a>
            .
          </p>
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
          {!isLoading && list.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">
              No DRS objects yet. Upload a file above or register via the API.
            </p>
          )}
          {!isLoading && list.length === 0 && error && (
            <p className="text-muted-foreground text-sm">No objects listed. Try upload or fix DRS configuration.</p>
          )}
          {!isLoading && list.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border">
                    <th className="py-2 text-left font-medium">ID</th>
                    <th className="py-2 text-left font-medium">Name</th>
                    <th className="py-2 text-left font-medium">Size</th>
                    <th className="py-2 text-left font-medium">Type</th>
                    <th className="py-2 text-left font-medium">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {list.map((obj) => (
                    <tr key={obj.id} className="border-b border-border/50">
                      <td className="py-2 font-mono text-xs">{obj.id}</td>
                      <td className="py-2">{obj.name ?? '—'}</td>
                      <td className="py-2">{obj.size != null ? `${(obj.size / 1024).toFixed(1)} KB` : '—'}</td>
                      <td className="py-2">{obj.mime_type ?? '—'}</td>
                      <td className="py-2">
                        <Link to={`/data/objects/${obj.id}` as any} className="text-primary hover:underline">
                          View
                        </Link>
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
