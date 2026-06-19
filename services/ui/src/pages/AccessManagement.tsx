import { useMemo, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { FederatedComputeRunDialog } from '@/components/FederatedComputeRunDialog';
import { RequestAccessDialog } from '@/components/RequestAccessDialog';
import { CreateProjectDialog } from '@/components/CreateProjectDialog';
import {
  listCatalogDatasets,
  listFederatedCatalog,
  listMyAccessRequests,
  listMyGrants,
  listMyProjects,
  federatedDrsUrl,
  getAccessStatus,
  type DatasetCatalogEntry,
} from '@/api/access';
import { useAuthStore } from '@/stores/auth';
import { decodeJwtPayload } from '@/lib/auth';
import { useI18n } from '@/i18n/I18nProvider';
import {
  Shield,
  Key,
  FileCheck,
  Settings,
  ExternalLink,
  Loader2,
  Plus,
  Database,
  ClipboardList,
  Server,
  Globe,
} from 'lucide-react';

function statusBadge(status: string) {
  const variant =
    status === 'approved'
      ? 'default'
      : status === 'pending' || status === 'escalated'
        ? 'secondary'
        : 'destructive';
  return <Badge variant={variant}>{status}</Badge>;
}

export function AccessManagement() {
  const { t } = useI18n();
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const researcherId = useMemo(() => {
    if (!passportJwt) return '';
    const claims = decodeJwtPayload(passportJwt);
    return typeof claims?.sub === 'string' ? claims.sub : '';
  }, [passportJwt]);

  const [requestDataset, setRequestDataset] = useState<DatasetCatalogEntry | null>(null);
  const [showNewProject, setShowNewProject] = useState(false);
  const [catalogKind, setCatalogKind] = useState<'datasets' | 'compute' | 'federated'>('datasets');

  const { data: status } = useQuery({
    queryKey: ['access', 'status'],
    queryFn: getAccessStatus,
    retry: false,
  });

  const adsAvailable = status?.ads_available ?? false;

  const { data: catalog, isLoading: catalogLoading } = useQuery({
    queryKey: ['access', 'catalog', catalogKind],
    queryFn: async () => {
      if (catalogKind === 'federated') {
        return (await listFederatedCatalog()).datasets;
      }
      const type = catalogKind === 'compute' ? 'compute_pool' : 'dataset';
      return (await listCatalogDatasets(type)).datasets;
    },
    enabled: adsAvailable,
    retry: false,
  });

  const { data: projects = [] } = useQuery({
    queryKey: ['access', 'projects'],
    queryFn: async () => (await listMyProjects()).projects,
    enabled: adsAvailable && !!researcherId,
    retry: false,
  });

  const { data: requests = [] } = useQuery({
    queryKey: ['access', 'requests'],
    queryFn: async () => (await listMyAccessRequests()).requests,
    enabled: adsAvailable && !!researcherId,
    retry: false,
  });

  const { data: grants = [] } = useQuery({
    queryKey: ['access', 'grants'],
    queryFn: async () => (await listMyGrants()).grants,
    enabled: adsAvailable && !!researcherId,
    retry: false,
  });

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t('access.title')}</h1>
        <p className="text-muted-foreground mt-1">{t('access.subtitle')}</p>
      </div>

      {!adsAvailable && (
        <Card className="border-amber-500/30 bg-amber-500/5">
          <CardContent className="pt-6 text-sm text-muted-foreground">
            {t('access.adsUnavailable')}
          </CardContent>
        </Card>
      )}

      {adsAvailable && (
        <Tabs defaultValue="catalog">
          <TabsList>
            <TabsTrigger value="catalog">{t('access.tabCatalog')}</TabsTrigger>
            <TabsTrigger value="requests">{t('access.tabRequests')}</TabsTrigger>
            <TabsTrigger value="projects">{t('access.tabProjects')}</TabsTrigger>
            <TabsTrigger value="grants">{t('access.tabGrants')}</TabsTrigger>
          </TabsList>

          <TabsContent value="catalog" className="mt-6 space-y-4">
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant={catalogKind === 'datasets' ? 'default' : 'outline'}
                onClick={() => setCatalogKind('datasets')}
              >
                <Database className="h-3.5 w-3.5 mr-1" />
                {t('access.catalogDatasets')}
              </Button>
              <Button
                size="sm"
                variant={catalogKind === 'compute' ? 'default' : 'outline'}
                onClick={() => setCatalogKind('compute')}
              >
                <Server className="h-3.5 w-3.5 mr-1" />
                {t('access.catalogCompute')}
              </Button>
              <Button
                size="sm"
                variant={catalogKind === 'federated' ? 'default' : 'outline'}
                onClick={() => setCatalogKind('federated')}
              >
                <Globe className="h-3.5 w-3.5 mr-1" />
                {t('access.catalogFederated')}
              </Button>
            </div>
            {catalogLoading ? (
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            ) : !catalog?.length ? (
              <p className="text-sm text-muted-foreground">
                {catalogKind === 'compute'
                  ? t('access.noComputePools')
                  : catalogKind === 'federated'
                    ? t('access.noFederated')
                    : t('access.noDatasets')}
              </p>
            ) : (
              <div className="grid gap-4 md:grid-cols-2">
                {catalog.map((ds) => (
                  <Card
                    key={ds.external_id ?? `${ds.federation_origin ?? 'local'}-${ds.id}`}
                    className="border-border/80"
                  >
                    <CardHeader className="pb-2">
                      <CardTitle className="flex items-center gap-2 text-base">
                        {catalogKind === 'compute' ? (
                          <Server className="h-4 w-4" />
                        ) : (
                          <Database className="h-4 w-4" />
                        )}
                        {ds.name}
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-3 text-sm">
                      {(ds as { federation_origin?: string }).federation_origin && (
                        <p className="text-xs text-muted-foreground">
                          {t('access.federationOrigin')}:{' '}
                          <span className="font-mono">
                            {(ds as { federation_origin?: string }).federation_origin}
                          </span>
                        </p>
                      )}
                      {ds.description && (
                        <p className="text-muted-foreground">{ds.description}</p>
                      )}
                      <div className="flex flex-wrap gap-1">
                        {ds.duo_codes.map((c) => (
                          <Badge key={c} variant="outline">
                            {c}
                          </Badge>
                        ))}
                      </div>
                      {ds.dac_group && (
                        <p className="text-xs text-muted-foreground">
                          {t('access.dacGroup')}: {ds.dac_group}
                        </p>
                      )}
                      {ds.external_id && (
                        <p className="text-xs font-mono text-muted-foreground truncate">
                          {ds.external_id}
                        </p>
                      )}
                      {(ds.remote_drs_base_url || ds.federation_origin) && ds.external_id && (
                        <p className="text-xs">
                          <a
                            href={
                              federatedDrsUrl(
                                ds.remote_drs_base_url,
                                ds.external_id,
                                ds.federation_origin,
                                ds.id,
                              ) ?? '#'
                            }
                            className="inline-flex items-center gap-1 text-primary hover:underline"
                          >
                            {t('access.remoteDrs')}
                            <ExternalLink className="h-3 w-3" />
                          </a>
                        </p>
                      )}
                      {ds.remote_wes_base_url && ds.resource_type === 'compute_pool' && (
                        <p className="text-xs font-mono text-muted-foreground truncate">
                          {t('access.remoteWes')}: {ds.remote_wes_base_url}
                        </p>
                      )}
                      <Button
                        size="sm"
                        disabled={!researcherId}
                        onClick={() => setRequestDataset(ds)}
                      >
                        {t('access.requestAccess')}
                      </Button>
                    </CardContent>
                  </Card>
                ))}
              </div>
            )}
          </TabsContent>

          <TabsContent value="requests" className="mt-6">
            {!requests.length ? (
              <p className="text-sm text-muted-foreground">{t('access.noRequests')}</p>
            ) : (
              <ul className="space-y-3">
                {requests.map((r) => (
                  <li key={r.id} className="rounded-lg border border-border p-4 text-sm">
                    <div className="flex flex-wrap items-center gap-2">
                      {statusBadge(r.status)}
                      <span className="text-muted-foreground font-mono text-xs">{r.id}</span>
                    </div>
                    <p className="mt-2">
                      {t('access.datasetId')}: <code className="rounded bg-muted px-1">{r.dataset_id}</code>
                    </p>
                    {r.justification && (
                      <p className="mt-1 text-muted-foreground">{r.justification}</p>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </TabsContent>

          <TabsContent value="projects" className="mt-6 space-y-4">
            <Button size="sm" onClick={() => setShowNewProject(true)} disabled={!researcherId}>
              <Plus className="h-4 w-4 mr-1" />
              {t('access.newProject')}
            </Button>
            {!projects.length ? (
              <p className="text-sm text-muted-foreground">{t('access.noProjects')}</p>
            ) : (
              <ul className="space-y-3">
                {projects.map((p) => (
                  <li key={p.id} className="rounded-lg border border-border p-4 text-sm">
                    <p className="font-medium">{p.name}</p>
                    {p.description && (
                      <p className="text-muted-foreground mt-1">{p.description}</p>
                    )}
                    <div className="mt-2 flex flex-wrap gap-1">
                      {p.duo_codes.map((c) => (
                        <Badge key={c} variant="outline">
                          {c}
                        </Badge>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </TabsContent>

          <TabsContent value="grants" className="mt-6">
            {!grants.length ? (
              <p className="text-sm text-muted-foreground">{t('access.noGrants')}</p>
            ) : (
              <ul className="space-y-3">
                {grants.map((g) => (
                  <li key={`${g.federation_origin ?? 'local'}-${g.id}`} className="rounded-lg border border-border p-4 text-sm">
                    <div className="flex flex-wrap items-center gap-2">
                      <FileCheck className="h-4 w-4 text-green-600" />
                      <span className="font-medium">
                        {g.dataset_name ?? t('access.activeGrant')}
                      </span>
                      {g.resource_type === 'compute_pool' && (
                        <Badge variant="secondary">{t('access.catalogCompute')}</Badge>
                      )}
                      {g.source && <Badge variant="outline">{g.source}</Badge>}
                    </div>
                    {g.federation_origin && g.federation_origin !== 'local' && (
                      <p className="mt-1 text-xs text-muted-foreground">
                        {t('access.federationOrigin')}:{' '}
                        <span className="font-mono">{g.federation_origin}</span>
                      </p>
                    )}
                    <p className="mt-2 font-mono text-xs text-muted-foreground">
                      {t('access.datasetId')}: {g.dataset_id}
                    </p>
                    {g.external_id && (
                      <p className="mt-1 font-mono text-xs text-muted-foreground truncate">
                        {g.external_id}
                      </p>
                    )}
                    <div className="mt-2 flex flex-wrap gap-1">
                      {g.duo_codes.map((c) => (
                        <Badge key={c} variant="outline">
                          {c}
                        </Badge>
                      ))}
                    </div>
                    {(g.remote_drs_base_url || g.federation_origin) && g.external_id && (
                      <p className="mt-2 text-xs">
                        <a
                          href={
                            federatedDrsUrl(
                              g.remote_drs_base_url,
                              g.external_id,
                              g.federation_origin,
                              g.dataset_id,
                            ) ?? '#'
                          }
                          className="inline-flex items-center gap-1 text-primary hover:underline"
                        >
                          {t('access.remoteDrs')}
                          <ExternalLink className="h-3 w-3" />
                        </a>
                      </p>
                    )}
                    {g.resource_type === 'compute_pool' &&
                      (g.remote_wes_base_url || g.federation_origin) && (
                        <div className="mt-2">
                          <FederatedComputeRunDialog grant={g} />
                        </div>
                      )}
                    {g.remote_wes_base_url && g.resource_type !== 'compute_pool' && (
                      <p className="mt-1 text-xs font-mono text-muted-foreground truncate">
                        {t('access.remoteWes')}: {g.remote_wes_base_url}
                      </p>
                    )}
                    {g.expires_at && (
                      <p className="mt-1 text-xs text-muted-foreground">
                        {t('access.grantExpires')}: {new Date(g.expires_at).toLocaleString()}
                      </p>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </TabsContent>
        </Tabs>
      )}

      <div className="grid gap-6 md:grid-cols-2">
        <Card className="border-border/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Key className="h-4 w-4" />
              {t('access.passportsTitle')}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground space-y-2">
            <p>{t('access.passportsBody')}</p>
          </CardContent>
        </Card>
        <Card className="border-border/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Shield className="h-4 w-4" />
              {t('access.standardsTitle')}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground space-y-2">
            <p>{t('access.standardsBody')}</p>
            <a
              href="https://github.com/ga4gh-duri/ga4gh-duri.github.io"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-primary hover:underline"
            >
              GA4GH DURI <ExternalLink className="h-3 w-3" />
            </a>
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap gap-3">
        <Link
          to={"/settings" as any}
          className="inline-flex items-center gap-2 rounded-md border border-border bg-card px-4 py-2 text-sm font-medium transition-colors hover:bg-muted"
        >
          <Settings className="h-4 w-4" />
          {t('access.openSettings')}
        </Link>
        <Link
          to={"/data" as any}
          className="inline-flex items-center gap-2 rounded-md border border-border bg-card px-4 py-2 text-sm font-medium transition-colors hover:bg-muted"
        >
          <ClipboardList className="h-4 w-4" />
          {t('access.dataBrowser')}
        </Link>
      </div>

      <RequestAccessDialog
        dataset={requestDataset}
        projects={projects}
        researcherId={researcherId}
        open={!!requestDataset}
        onOpenChange={(o) => !o && setRequestDataset(null)}
      />
      <CreateProjectDialog
        researcherId={researcherId}
        open={showNewProject}
        onOpenChange={setShowNewProject}
      />
    </div>
  );
}
