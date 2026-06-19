import { useMemo, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { RequestAccessDialog } from '@/components/RequestAccessDialog';
import { CreateProjectDialog } from '@/components/CreateProjectDialog';
import { FederatedComputeRunDialog } from '@/components/FederatedComputeRunDialog';
import {
  federatedDrsUrl,
  getAccessStatus,
  listCatalogDatasets,
  listFederatedCatalog,
  listMyGrants,
  listMyProjects,
  type DatasetCatalogEntry,
  type Grant,
} from '@/api/access';
import { useAuthStore } from '@/stores/auth';
import { decodeJwtPayload } from '@/lib/auth';
import { useI18n } from '@/i18n/I18nProvider';
import { Database, ExternalLink, Globe, Loader2, Plus, Server, Shield } from 'lucide-react';

type CatalogKind = 'institute' | 'compute' | 'federated';

type FederatedEntry = DatasetCatalogEntry & {
  federation_origin?: string;
  ads_base_url?: string;
};

function datasetKey(ds: FederatedEntry) {
  return ds.external_id ?? `${ds.federation_origin ?? 'local'}-${ds.id}`;
}

function grantForEntry(grants: Grant[], ds: DatasetCatalogEntry): Grant | undefined {
  return grants.find(
    (g) =>
      g.resource_type === 'compute_pool' &&
      (g.dataset_id === ds.id || g.dataset_id === ds.external_id),
  );
}

export function DatasetCatalogPanel({ compact }: { compact?: boolean }) {
  const { t } = useI18n();
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const researcherId = useMemo(() => {
    if (!passportJwt) return '';
    const claims = decodeJwtPayload(passportJwt);
    return typeof claims?.sub === 'string' ? claims.sub : '';
  }, [passportJwt]);

  const [catalogKind, setCatalogKind] = useState<CatalogKind>('institute');
  const [requestDataset, setRequestDataset] = useState<DatasetCatalogEntry | null>(null);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [pendingRequestAfterProject, setPendingRequestAfterProject] = useState<DatasetCatalogEntry | null>(null);

  const { data: status } = useQuery({
    queryKey: ['access', 'status'],
    queryFn: getAccessStatus,
    retry: false,
  });

  const adsAvailable = status?.ads_available ?? false;

  const { data: instituteCatalog = [], isLoading: instituteLoading } = useQuery({
    queryKey: ['access', 'catalog', 'datasets', 'data-browser'],
    queryFn: async () => (await listCatalogDatasets('dataset')).datasets,
    enabled: adsAvailable && catalogKind === 'institute',
    retry: false,
  });

  const { data: computeCatalog = [], isLoading: computeLoading } = useQuery({
    queryKey: ['access', 'catalog', 'compute', 'data-browser'],
    queryFn: async () => (await listCatalogDatasets('compute_pool')).datasets,
    enabled: adsAvailable && catalogKind === 'compute',
    retry: false,
  });

  const { data: federatedCatalog = [], isLoading: federatedLoading } = useQuery({
    queryKey: ['access', 'catalog', 'federated', 'data-browser'],
    queryFn: async () => (await listFederatedCatalog()).datasets,
    enabled: adsAvailable && catalogKind === 'federated',
    retry: false,
  });

  const { data: projects = [] } = useQuery({
    queryKey: ['access', 'projects'],
    queryFn: async () => (await listMyProjects()).projects,
    enabled: adsAvailable && !!researcherId,
    retry: false,
  });

  const { data: grants = [] } = useQuery({
    queryKey: ['access', 'grants'],
    queryFn: async () => (await listMyGrants()).grants,
    enabled: adsAvailable && !!researcherId,
    retry: false,
  });

  const catalog =
    catalogKind === 'institute'
      ? instituteCatalog
      : catalogKind === 'compute'
        ? computeCatalog
        : federatedCatalog;
  const isLoading =
    catalogKind === 'institute'
      ? instituteLoading
      : catalogKind === 'compute'
        ? computeLoading
        : federatedLoading;

  const emptyMessage =
    catalogKind === 'compute'
      ? t('access.noComputePools')
      : catalogKind === 'federated'
        ? t('access.noFederated')
        : t('access.noDatasets');

  const openRequest = (ds: DatasetCatalogEntry) => {
    if (projects.length === 0 && researcherId) {
      setPendingRequestAfterProject(ds);
      setShowCreateProject(true);
      return;
    }
    setRequestDataset(ds);
  };

  if (!adsAvailable) {
    return (
      <Card className="border-amber-500/30 bg-amber-500/5">
        <CardContent className="pt-6 space-y-3 text-sm text-muted-foreground">
          <p>{t('data.catalogUnavailable')}</p>
          {!compact && (
            <Button asChild variant="outline" size="sm" className="gap-1">
              <Link to={'/access' as any}>
                <Shield className="h-3.5 w-3.5" />
                {t('data.openAccessManagement')}
              </Link>
            </Button>
          )}
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">{t('data.catalogHint')}</p>

      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant={catalogKind === 'institute' ? 'default' : 'outline'}
          onClick={() => setCatalogKind('institute')}
        >
          <Database className="h-3.5 w-3.5 mr-1" />
          {t('data.catalogInstitute')}
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
          {t('data.catalogFederated')}
        </Button>
      </div>

      {catalogKind === 'compute' && (
        <p className="text-xs text-muted-foreground">{t('access.computeHint')}</p>
      )}

      {!researcherId && (
        <p className="text-sm text-amber-600 dark:text-amber-400">{t('data.catalogSignIn')}</p>
      )}

      {researcherId && projects.length === 0 && (
        <Card className="border-dashed">
          <CardContent className="pt-4 flex flex-wrap items-center justify-between gap-3">
            <p className="text-sm text-muted-foreground">{t('data.catalogNoProject')}</p>
            <Button size="sm" className="gap-1" onClick={() => setShowCreateProject(true)}>
              <Plus className="h-3.5 w-3.5" />
              {t('data.createProject')}
            </Button>
          </CardContent>
        </Card>
      )}

      {isLoading ? (
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      ) : catalog.length === 0 ? (
        <p className="text-sm text-muted-foreground">{emptyMessage}</p>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {catalog.map((ds) => {
            const fed = ds as FederatedEntry;
            const isCompute = ds.resource_type === 'compute_pool' || catalogKind === 'compute';
            const activeGrant = isCompute ? grantForEntry(grants, ds) : undefined;
            const remoteUrl =
              fed.remote_drs_base_url && fed.external_id
                ? federatedDrsUrl(
                    fed.remote_drs_base_url,
                    fed.external_id,
                    fed.federation_origin,
                    fed.id,
                  )
                : null;
            return (
              <Card key={datasetKey(fed)} className="border-border/80">
                <CardContent className="pt-4 space-y-3 text-sm">
                  <p className="font-medium flex items-center gap-2">
                    {isCompute ? (
                      <Server className="h-4 w-4 shrink-0" />
                    ) : catalogKind === 'federated' ? (
                      <Globe className="h-4 w-4 shrink-0" />
                    ) : (
                      <Database className="h-4 w-4 shrink-0" />
                    )}
                    {ds.name}
                    {isCompute && (
                      <Badge variant="secondary" className="text-xs">
                        {t('access.catalogCompute')}
                      </Badge>
                    )}
                  </p>
                  {fed.federation_origin && (
                    <p className="text-xs text-muted-foreground">
                      {t('access.federationOrigin')}:{' '}
                      <span className="font-mono">{fed.federation_origin}</span>
                    </p>
                  )}
                  {ds.description && <p className="text-muted-foreground">{ds.description}</p>}
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
                  {ds.visibility && (
                    <p className="text-xs text-muted-foreground">
                      {t('data.catalogVisibility')}: {ds.visibility}
                    </p>
                  )}
                  {ds.external_id && (
                    <p className="text-xs font-mono text-muted-foreground truncate">{ds.external_id}</p>
                  )}
                  {remoteUrl && (
                    <a
                      href={remoteUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
                    >
                      {t('access.remoteDrs')}
                      <ExternalLink className="h-3 w-3" />
                    </a>
                  )}
                  {ds.remote_wes_base_url && (
                    <p className="text-xs font-mono text-muted-foreground truncate">
                      {t('access.remoteWes')}: {ds.remote_wes_base_url}
                    </p>
                  )}
                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      disabled={!researcherId}
                      onClick={() => openRequest(ds)}
                    >
                      {t('access.requestAccess')}
                    </Button>
                    {activeGrant ? (
                      <FederatedComputeRunDialog grant={activeGrant} />
                    ) : (
                      isCompute &&
                      ds.remote_wes_base_url &&
                      researcherId && (
                        <p className="text-xs text-muted-foreground self-center">
                          {t('access.computeGrantRequired')}
                        </p>
                      )
                    )}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      <Button asChild variant="outline" size="sm" className="gap-1">
        <Link to={'/access' as any}>
          <ExternalLink className="h-3.5 w-3.5" />
          {t('data.openAccessManagement')}
        </Link>
      </Button>

      <CreateProjectDialog
        researcherId={researcherId}
        open={showCreateProject}
        onOpenChange={setShowCreateProject}
        onCreated={() => {
          if (pendingRequestAfterProject) {
            setRequestDataset(pendingRequestAfterProject);
            setPendingRequestAfterProject(null);
          }
        }}
      />

      <RequestAccessDialog
        dataset={requestDataset}
        projects={projects}
        researcherId={researcherId}
        open={!!requestDataset}
        onOpenChange={(open) => !open && setRequestDataset(null)}
      />
    </div>
  );
}
