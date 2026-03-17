import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { apiGet } from '@/api/client';
import { Database, Upload, AlertCircle } from 'lucide-react';

interface DrsObject {
  id: string;
  name?: string;
  description?: string;
  size?: number;
  mime_type?: string;
  created_time?: string;
}

export function DataBrowser() {
  const { data: objects, isLoading, error } = useQuery({
    queryKey: ['drs', 'objects'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    retry: false,
  });

  const list = Array.isArray(objects) ? objects : [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Data Browser</h1>
          <p className="text-muted-foreground">Browse and manage DRS objects.</p>
        </div>
        <Button variant="outline" disabled className="gap-2">
          <Upload className="h-4 w-4" />
          Upload
        </Button>
      </div>
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          DRS is not configured or unavailable. Start the gateway with storage and DRS enabled to see objects here.
        </div>
      )}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            Objects
          </CardTitle>
          <p className="text-sm text-muted-foreground">
            Ingest files via the DRS API or CLI; upload UI is coming in a future release.
          </p>
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
          {!isLoading && list.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">No DRS objects yet. Use the API or CLI to register objects.</p>
          )}
          {!isLoading && list.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border">
                    <th className="text-left py-2 font-medium">ID</th>
                    <th className="text-left py-2 font-medium">Name</th>
                    <th className="text-left py-2 font-medium">Size</th>
                    <th className="text-left py-2 font-medium">Type</th>
                    <th className="text-left py-2 font-medium">Actions</th>
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
