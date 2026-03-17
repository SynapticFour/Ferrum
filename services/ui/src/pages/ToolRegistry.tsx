import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { apiGet } from '@/api/client';
import { Wrench, AlertCircle } from 'lucide-react';

interface Tool {
  id: string;
  name?: string;
  description?: string;
  organization?: string;
  toolclass?: { id?: string; name?: string };
  meta_version?: string;
}

interface ToolListResponse {
  tools: Tool[];
  next_page_token?: string;
}

export function ToolRegistry() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['trs', 'tools'],
    queryFn: () => apiGet<ToolListResponse>('/ga4gh/trs/v2/tools'),
    retry: false,
  });

  const tools = data?.tools ?? [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Tool Registry</h1>
        <p className="text-muted-foreground">Registered tools from GA4GH TRS (e.g. workflows, containers).</p>
      </div>
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          TRS is not configured or unavailable. Start the gateway with a database to see tools here.
        </div>
      )}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Wrench className="h-4 w-4" />
            Tools
          </CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
          {!isLoading && tools.length === 0 && !error && (
            <p className="text-muted-foreground text-sm">No tools registered. Add tools via the TRS API or seed demo data.</p>
          )}
          {!isLoading && tools.length > 0 && (
            <ul className="space-y-3">
              {tools.map((t) => (
                <li key={t.id} className="rounded-lg border border-border p-4">
                  <p className="font-medium">{t.name ?? t.id}</p>
                  {t.description && <p className="text-sm text-muted-foreground mt-1">{t.description}</p>}
                  <p className="text-xs text-muted-foreground mt-2">
                    ID: <code className="rounded bg-muted px-1">{t.id}</code>
                    {t.organization && ` · ${t.organization}`}
                    {t.toolclass?.name && ` · ${t.toolclass.name}`}
                  </p>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
