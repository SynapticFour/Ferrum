import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiGet, apiPost } from '@/api/client';
import { loadFederationPrefs, saveFederationPrefs } from '@/stores/federation';
import { Globe, Loader2, Network, AlertCircle, CheckCircle2 } from 'lucide-react';

interface FederationStatus {
  discovery_enabled: boolean;
  auto_register: boolean;
  service_registry_url?: string;
  registration_base_url?: string;
  public_base_url?: string;
  services: {
    drs: boolean;
    beacon: boolean;
    htsget: boolean;
    wes: boolean;
    tes: boolean;
    trs: boolean;
  };
}

interface RegisteredService {
  id: string;
  name: string;
  url: string;
  type: { artifact: string; version: string };
}

export function FederationPanel() {
  const qc = useQueryClient();
  const [prefs, setPrefs] = useState(loadFederationPrefs);
  const [apiKey, setApiKey] = useState('');
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    saveFederationPrefs(prefs);
  }, [prefs]);

  const { data: status, isLoading: statusLoading } = useQuery({
    queryKey: ['admin', 'federation', 'status'],
    queryFn: () => apiGet<FederationStatus>('/admin/federation/status'),
    retry: false,
  });

  const listMutation = useMutation({
    mutationFn: () =>
      apiPost<{ services: RegisteredService[] }>('/admin/federation/registry/services', {
        registry_url: prefs.registryUrl,
        api_key: apiKey || undefined,
      }),
    onSuccess: () => {
      setMessage('Registry services loaded.');
      qc.invalidateQueries({ queryKey: ['admin', 'federation', 'services'] });
    },
    onError: (e: Error) => setMessage(e.message),
  });

  const { data: registryData, isFetching: registryLoading } = useQuery({
    queryKey: ['admin', 'federation', 'services', prefs.registryUrl],
    queryFn: () =>
      apiPost<{ services: RegisteredService[] }>('/admin/federation/registry/services', {
        registry_url: prefs.registryUrl,
        api_key: apiKey || undefined,
      }),
    enabled: false,
  });

  const registerMutation = useMutation({
    mutationFn: () =>
      apiPost<{ registered: string[] }>('/admin/federation/registry/register-node', {
        registry_url: prefs.registryUrl,
        api_key: apiKey,
        public_base_url: prefs.publicBaseUrl,
        node_id_prefix: prefs.nodeIdPrefix,
        organization_name: prefs.organizationName,
      }),
    onSuccess: (res) => setMessage(`Registered: ${res.registered.join(', ')}`),
    onError: (e: Error) => setMessage(e.message),
  });

  const services = registryData?.services ?? [];

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Network className="h-4 w-4" />
            Join GA4GH federation
          </CardTitle>
          <p className="text-sm text-muted-foreground">
            Connect this Ferrum node to a ga4gh-infra service registry. List peers, then register
            this node&apos;s DRS/Beacon/WES/TRS endpoints. Requires a reachable public URL (tunnel if
            on a laptop).
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          {statusLoading && <p className="text-sm text-muted-foreground">Loading status…</p>}
          {status && (
            <div className="rounded-md border border-border bg-muted/30 p-3 text-sm space-y-1">
              <p>
                Server auto-register:{' '}
                <strong>{status.auto_register ? 'on' : 'off'}</strong>
                {status.service_registry_url && (
                  <> · Registry: <code className="text-xs">{status.service_registry_url}</code></>
                )}
              </p>
              <p className="text-muted-foreground text-xs">
                Enabled services — DRS: {status.services.drs ? 'yes' : 'no'} · Beacon:{' '}
                {status.services.beacon ? 'yes' : 'no'} · TRS: {status.services.trs ? 'yes' : 'no'}{' '}
                · WES: {status.services.wes ? 'yes' : 'no'}
              </p>
            </div>
          )}

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="registry-url">Service registry URL</Label>
              <Input
                id="registry-url"
                value={prefs.registryUrl}
                onChange={(e) => setPrefs({ ...prefs, registryUrl: e.target.value })}
                placeholder="https://pasteur-pilot-ga4gh-infra.fly.dev"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="api-key">Registration API key</Label>
              <Input
                id="api-key"
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="Not stored — enter each session"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="public-url">This node public URL</Label>
              <Input
                id="public-url"
                value={prefs.publicBaseUrl}
                onChange={(e) => setPrefs({ ...prefs, publicBaseUrl: e.target.value })}
                placeholder="https://your-tunnel.example.com"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="node-prefix">Node ID prefix</Label>
              <Input
                id="node-prefix"
                value={prefs.nodeIdPrefix}
                onChange={(e) => setPrefs({ ...prefs, nodeIdPrefix: e.target.value })}
              />
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              disabled={!prefs.registryUrl || listMutation.isPending}
              onClick={() => listMutation.mutate()}
              className="gap-2"
            >
              {listMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Globe className="h-4 w-4" />
              )}
              List registry services
            </Button>
            <Button
              type="button"
              disabled={!prefs.registryUrl || !apiKey || registerMutation.isPending}
              onClick={() => registerMutation.mutate()}
              className="gap-2"
            >
              {registerMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <CheckCircle2 className="h-4 w-4" />
              )}
              Register this node
            </Button>
          </div>

          {message && (
            <p className="text-sm text-muted-foreground">{message}</p>
          )}

          {registryLoading && (
            <p className="text-sm text-muted-foreground">Loading registry…</p>
          )}
          {services.length > 0 && (
            <ul className="space-y-2 text-sm">
              {services.map((s) => (
                <li key={s.id} className="rounded border border-border p-3">
                  <p className="font-medium">{s.name}</p>
                  <p className="text-xs text-muted-foreground">
                    {s.type.artifact} · <code>{s.url}</code>
                  </p>
                </li>
              ))}
            </ul>
          )}

          <div className="flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-sm text-amber-800 dark:text-amber-200">
            <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
            <p>
              For persistent auto-register on startup, set{' '}
              <code className="text-xs">FERRUM_DISCOVERY__ENABLED=true</code>,{' '}
              <code className="text-xs">FERRUM_DISCOVERY__AUTO_REGISTER=true</code>, and{' '}
              <code className="text-xs">SERVICE_REGISTRY_REGISTRATION_KEY</code> in the gateway
              environment, then restart Ferrum.
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
