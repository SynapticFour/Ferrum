import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiGet } from '@/api/client';
import { Key, Database, Server, User, HardDrive, Info } from 'lucide-react';

interface SanitizedConfig {
  bind?: string;
  database?: { driver?: string; url_set?: boolean; run_migrations?: boolean; max_connections?: number };
  storage?: { backend?: string; s3_endpoint?: string; s3_bucket?: string };
  services?: {
    enable_drs?: boolean;
    enable_wes?: boolean;
    enable_tes?: boolean;
    enable_trs?: boolean;
    enable_beacon?: boolean;
    enable_passports?: boolean;
    enable_crypt4gh?: boolean;
  };
  message?: string;
}

interface CacheStats {
  total_entries: number;
  total_size_bytes: number;
  hit_rate_7d: number;
  top_cached_tasks?: Array<{ task_name: string; hits: number; size_gb: number }>;
}

export function Settings() {
  const { data: config, isLoading: configLoading, error: configError } = useQuery({
    queryKey: ['admin', 'config'],
    queryFn: () => apiGet<SanitizedConfig>('/admin/config'),
    retry: false,
  });
  const { data: cacheStats } = useQuery({
    queryKey: ['wes', 'cache', 'stats'],
    queryFn: () => apiGet<CacheStats>('/ga4gh/wes/v1/cache/stats'),
    retry: false,
  });

  const hasConfig = config && !('message' in config && config.message);
  const storage = config?.storage;
  const db = config?.database;
  const services = config?.services;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">Server configuration, storage, and profile.</p>
      </div>

      <Tabs defaultValue="config">
        <TabsList>
          <TabsTrigger value="config">Server</TabsTrigger>
          <TabsTrigger value="storage">Storage</TabsTrigger>
          <TabsTrigger value="keys">Encryption keys</TabsTrigger>
          <TabsTrigger value="cache">Workflow cache</TabsTrigger>
          <TabsTrigger value="profile">Profile</TabsTrigger>
        </TabsList>
        <TabsContent value="config">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Server className="h-4 w-4" />
                Server configuration
              </CardTitle>
              <p className="text-sm text-muted-foreground">
                Read-only view of bind address, database, and enabled services. Secrets are not shown.
              </p>
            </CardHeader>
            <CardContent>
              {configLoading && <p className="text-muted-foreground text-sm">Loading…</p>}
              {configError && (
                <p className="text-destructive text-sm">
                  Could not load config. The gateway may not expose /admin/config or the service may be unavailable.
                </p>
              )}
              {hasConfig && (
                <div className="space-y-4 text-sm">
                  <div>
                    <span className="font-medium text-muted-foreground">Bind</span>
                    <p className="font-mono">{config.bind ?? '—'}</p>
                  </div>
                  <div>
                    <span className="font-medium text-muted-foreground">Database</span>
                    <p>
                      Driver: <code className="rounded bg-muted px-1">{db?.driver ?? '—'}</code>
                      {db?.url_set != null && (
                        <span className="ml-2">URL set: {db.url_set ? 'yes' : 'no'}</span>
                      )}
                      {' · '}
                      Migrations: {db?.run_migrations ? 'on' : 'off'}
                      {' · '}
                      Max connections: {db?.max_connections ?? '—'}
                    </p>
                  </div>
                  <div>
                    <span className="font-medium text-muted-foreground">Services</span>
                    <p className="flex flex-wrap gap-x-4 gap-y-1">
                      {services && (
                        <>
                          DRS: {services.enable_drs ? 'on' : 'off'} · WES: {services.enable_wes ? 'on' : 'off'} · TES: {services.enable_tes ? 'on' : 'off'} · TRS: {services.enable_trs ? 'on' : 'off'} · Beacon: {services.enable_beacon ? 'on' : 'off'} · Passports: {services.enable_passports ? 'on' : 'off'} · Crypt4GH: {services.enable_crypt4gh ? 'on' : 'off'}
                        </>
                      )}
                    </p>
                  </div>
                  <details className="mt-2">
                    <summary className="cursor-pointer text-muted-foreground">Raw JSON</summary>
                    <pre className="mt-2 overflow-auto rounded-md bg-muted p-4 text-xs">
                      {JSON.stringify(config, null, 2)}
                    </pre>
                  </details>
                </div>
              )}
              {!hasConfig && !configLoading && !configError && (
                <p className="text-muted-foreground text-sm">
                  Configuration not available (no config file or env). Start the gateway with FERRUM_DATABASE__URL and other env vars, or a config file.
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="storage">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <HardDrive className="h-4 w-4" />
                Storage backend
              </CardTitle>
              <p className="text-sm text-muted-foreground">
                DRS and workflow artifacts use this backend. Status is derived from server config.
              </p>
            </CardHeader>
            <CardContent>
              {hasConfig && storage ? (
                <div className="space-y-2 text-sm">
                  <p>
                    <span className="font-medium text-muted-foreground">Backend:</span>{' '}
                    <code className="rounded bg-muted px-1">{storage.backend ?? 'local'}</code>
                  </p>
                  {storage.s3_endpoint && (
                    <p>
                      <span className="font-medium text-muted-foreground">S3 endpoint:</span>{' '}
                      <code className="rounded bg-muted px-1">{storage.s3_endpoint}</code>
                    </p>
                  )}
                  {storage.s3_bucket && (
                    <p>
                      <span className="font-medium text-muted-foreground">S3 bucket:</span>{' '}
                      <code className="rounded bg-muted px-1">{storage.s3_bucket}</code>
                    </p>
                  )}
                  <p className="text-muted-foreground mt-2">
                    To check connectivity or quota, use the Data Browser or gateway health/ready endpoints.
                  </p>
                </div>
              ) : (
                <p className="text-muted-foreground text-sm">
                  Storage config not available. Configure the gateway with storage settings to see them here.
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="keys">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Key className="h-4 w-4" />
                Encryption keys
              </CardTitle>
              <p className="text-sm text-muted-foreground">
                View key IDs and manage keys for Crypt4GH or other encryption. Private keys are never shown in the UI.
              </p>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Key generation and rotation are done via the <strong>Ferrum CLI</strong> or admin API, not in this UI.
                Use <code className="rounded bg-muted px-1">ferrum-gateway</code> or your deployment tooling to generate
                or import keys; configure paths or key IDs in the server config (e.g. <code className="rounded bg-muted px-1">FERRUM_ENCRYPTION__*</code>).
              </p>
              <div className="rounded-md border border-border bg-muted/30 p-4 text-sm">
                <p className="font-medium flex items-center gap-2">
                  <Info className="h-4 w-4" />
                  No key management in UI
                </p>
                <p className="mt-1 text-muted-foreground">
                  To generate or list keys, use the gateway’s encryption APIs (if exposed) or the ferrum CLI. This page is for reference only.
                </p>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="cache">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Database className="h-4 w-4" />
                Workflow cache
              </CardTitle>
              <p className="text-sm text-muted-foreground">
                WES task cache statistics. Shown when the WES service and cache are enabled.
              </p>
            </CardHeader>
            <CardContent>
              {cacheStats != null ? (
                <div className="space-y-2 text-sm">
                  <p><span className="font-medium">Total entries:</span> {cacheStats.total_entries}</p>
                  <p><span className="font-medium">Total size:</span> {(cacheStats.total_size_bytes / 1e9).toFixed(2)} GB</p>
                  <p><span className="font-medium">7-day hit rate:</span> {(cacheStats.hit_rate_7d * 100).toFixed(1)}%</p>
                  {cacheStats.top_cached_tasks?.length ? (
                    <div className="mt-4">
                      <p className="font-medium mb-2">Top cached tasks</p>
                      <ul className="list-disc pl-4">
                        {cacheStats.top_cached_tasks.slice(0, 10).map((t, i) => (
                          <li key={i}>{t.task_name}: {t.hits} hits, {t.size_gb.toFixed(1)} GB</li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </div>
              ) : (
                <p className="text-muted-foreground text-sm">
                  Cache stats not available. Enable WES and the task cache in the gateway to see statistics here.
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="profile">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <User className="h-4 w-4" />
                Profile
              </CardTitle>
              <p className="text-sm text-muted-foreground">
                User profile and passport/visa viewer. When auth is enabled, your claims and visas appear here.
              </p>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                When the gateway is configured with an identity provider (e.g. Keycloak), sign in to see your profile.
                The UI stores the Passport JWT in memory (<code className="rounded bg-muted px-1">__ferrumPassport</code>) and sends it with API requests.
              </p>
              <div className="rounded-md border border-border bg-muted/30 p-4 text-sm">
                <p className="font-medium">No session</p>
                <p className="mt-1 text-muted-foreground">
                  You are not signed in or auth is disabled. Configure <code className="rounded bg-muted px-1">FERRUM_AUTH__*</code> and use your IdP to log in; then your visas and identity will be shown here.
                </p>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
