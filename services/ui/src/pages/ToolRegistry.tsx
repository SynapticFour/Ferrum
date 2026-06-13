import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiGet } from '@/api/client';
import { loadFederationPrefs } from '@/stores/federation';
import { Wrench, AlertCircle } from 'lucide-react';

interface Tool {
  id: string;
  name?: string;
  description?: string;
  organization?: string;
  toolclass?: { id?: string; name?: string };
}

type ToolListResponse = Tool[] | { tools: Tool[] };

interface RegisteredService {
  id: string;
  name: string;
  url: string;
  type: { artifact: string };
}

function ToolList({ tools, empty }: { tools: Tool[]; empty: string }) {
  if (tools.length === 0) {
    return <p className="text-muted-foreground text-sm">{empty}</p>;
  }
  return (
    <ul className="space-y-3">
      {tools.map((t) => (
        <li key={t.id} className="rounded-lg border border-border p-4">
          <p className="font-medium">{t.name ?? t.id}</p>
          {t.description && <p className="text-sm text-muted-foreground mt-1">{t.description}</p>}
          <p className="text-xs text-muted-foreground mt-2">
            ID: <code className="rounded bg-muted px-1">{t.id}</code>
            {t.organization && ` · ${t.organization}`}
          </p>
        </li>
      ))}
    </ul>
  );
}

export function ToolRegistry() {
  const [tab, setTab] = useState('local');
  const prefs = loadFederationPrefs();

  const { data, isLoading, error } = useQuery({
    queryKey: ['trs', 'tools', 'local'],
    queryFn: () => apiGet<ToolListResponse>('/ga4gh/trs/v2/tools'),
    retry: false,
  });

  const { data: registryServices } = useQuery({
    queryKey: ['federation', 'registry', prefs.registryUrl],
    queryFn: () =>
      fetch('/admin/federation/registry/services', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ registry_url: prefs.registryUrl }),
      }).then((r) => r.json()) as Promise<{ services: RegisteredService[] }>,
    enabled: tab === 'federation' && !!prefs.registryUrl,
    retry: false,
  });

  const trsServices =
    registryServices?.services.filter((s) => s.type.artifact === 'tool-registry') ?? [];
  const remoteTrs = trsServices[0]?.url;

  const { data: remoteTools, isLoading: remoteLoading } = useQuery({
    queryKey: ['trs', 'remote', remoteTrs],
    queryFn: () =>
      apiGet<{ trs_base_url?: string; tools: Tool[] | { tools?: Tool[] } }>(
        `/admin/federation/proxy/trs/tools?trs_base_url=${encodeURIComponent(remoteTrs!)}`
      ).then((r) => {
        const t = r.tools;
        if (Array.isArray(t)) return t;
        if (t && typeof t === 'object' && Array.isArray(t.tools)) return t.tools;
        return [];
      }),
    enabled: tab === 'federation' && !!remoteTrs,
    retry: false,
  });

  const localTools = Array.isArray(data) ? data : (data?.tools ?? []);
  const federatedTools = Array.isArray(remoteTools) ? remoteTools : [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Tool Registry</h1>
        <p className="text-muted-foreground">
          Browse workflows from local TRS or federation peers (via service registry).
        </p>
      </div>

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="local">Local</TabsTrigger>
          <TabsTrigger value="federation">Federation</TabsTrigger>
        </TabsList>
        <TabsContent value="local">
          {error && (
            <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400 mb-4">
              <AlertCircle className="h-4 w-4 shrink-0" />
              TRS unavailable — use Laptop Mode with DRS/Beacon only, or start full gateway with Postgres.
            </div>
          )}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Wrench className="h-4 w-4" />
                Local tools
              </CardTitle>
            </CardHeader>
            <CardContent>
              {isLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
              <ToolList tools={localTools} empty="No local tools. Register via TRS API or run ferrum demo seed." />
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="federation">
          <Card>
            <CardHeader>
              <CardTitle>Federation tools</CardTitle>
              <p className="text-sm text-muted-foreground">
                Loads TRS from the first tool-registry entry in Settings → Federation registry URL (
                {prefs.registryUrl || 'not configured'}).
              </p>
            </CardHeader>
            <CardContent>
              {!prefs.registryUrl && (
                <p className="text-sm text-muted-foreground">
                  Set a registry URL under Settings → Federation first.
                </p>
              )}
              {remoteLoading && <p className="text-sm text-muted-foreground">Loading remote TRS…</p>}
              <ToolList
                tools={federatedTools}
                empty={
                  remoteTrs
                    ? 'No tools on remote TRS.'
                    : 'No tool-registry service found in registry.'
                }
              />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
