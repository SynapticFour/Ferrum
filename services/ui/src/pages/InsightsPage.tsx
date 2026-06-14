import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { apiGet } from '@/api/client';
import { ServiceHealthPanel } from '@/components/ServiceHealthPanel';
import { useI18n } from '@/i18n/I18nProvider';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { formatBytes } from '@/lib/utils';
import {
  BarChart3,
  Database,
  FolderOpen,
  Globe,
  Shield,
  Users,
  Workflow,
} from 'lucide-react';

interface FederationStatus {
  discovery_enabled: boolean;
  auto_register: boolean;
  service_registry_url?: string;
}

interface DrsObject {
  id: string;
  size?: number;
}

interface SecurityEvents {
  events: unknown[];
}

export function InsightsPage() {
  const { t } = useI18n();
  const { data: config } = useAdminConfig();

  const { data: workspaces } = useQuery({
    queryKey: ['insights', 'workspaces'],
    queryFn: () => apiGet<{ id: string; name: string }[]>('/workspaces/v1/workspaces'),
    retry: false,
  });

  const { data: objects } = useQuery({
    queryKey: ['insights', 'drs'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects?limit=500'),
    retry: false,
  });

  const { data: cohorts } = useQuery({
    queryKey: ['insights', 'cohorts'],
    queryFn: () => apiGet<{ cohorts: { id: string }[] }>('/cohorts/v1/cohorts?limit=100'),
    retry: false,
  });

  const { data: runs } = useQuery({
    queryKey: ['insights', 'runs'],
    queryFn: () => apiGet<{ runs: { run_id: string; state?: string }[] }>('/ga4gh/wes/v1/runs?page_size=100'),
    retry: false,
  });

  const { data: federation } = useQuery({
    queryKey: ['insights', 'federation'],
    queryFn: () => apiGet<FederationStatus>('/admin/federation/status'),
    retry: false,
  });

  const { data: security } = useQuery({
    queryKey: ['insights', 'security'],
    queryFn: () => apiGet<SecurityEvents>('/admin/security/events?limit=5'),
    retry: false,
  });

  const wsList = Array.isArray(workspaces) ? workspaces : [];
  const objList = Array.isArray(objects) ? objects : [];
  const totalBytes = objList.reduce((s, o) => s + (o.size ?? 0), 0);
  const cohortCount = cohorts?.cohorts?.length ?? 0;
  const runList = runs?.runs ?? [];
  const completeRuns = runList.filter((r) => r.state === 'COMPLETE').length;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t('insights.title')}</h1>
        <p className="text-muted-foreground">{t('insights.subtitle')}</p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><FolderOpen className="h-4 w-4" />{t('insights.workspaces')}</CardTitle></CardHeader>
          <CardContent><p className="text-3xl font-bold">{wsList.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Database className="h-4 w-4" />{t('insights.dataObjects')}</CardTitle></CardHeader>
          <CardContent>
            <p className="text-3xl font-bold">{objList.length}</p>
            <p className="text-xs text-muted-foreground">{formatBytes(totalBytes)} indexed</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Users className="h-4 w-4" />{t('insights.cohorts')}</CardTitle></CardHeader>
          <CardContent><p className="text-3xl font-bold">{cohortCount}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm flex items-center gap-2"><Workflow className="h-4 w-4" />{t('insights.runs')}</CardTitle></CardHeader>
          <CardContent>
            <p className="text-3xl font-bold">{runList.length}</p>
            <p className="text-xs text-muted-foreground">{completeRuns} {t('insights.complete')}</p>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Globe className="h-4 w-4" />
              {t('insights.federation')}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm space-y-2">
            {federation ? (
              <>
                <p>{t('insights.discovery')}: <strong>{federation.discovery_enabled ? t('common.on') : t('common.off')}</strong></p>
                <p>{t('insights.autoRegister')}: <strong>{federation.auto_register ? t('common.on') : t('common.off')}</strong></p>
                {federation.service_registry_url && (
                  <p className="text-xs text-muted-foreground break-all">{federation.service_registry_url}</p>
                )}
              </>
            ) : (
              <p className="text-muted-foreground">{t('insights.federationUnavailable')}</p>
            )}
            <Button asChild size="sm" variant="outline">
              <Link to={'/settings' as any}>{t('insights.openFederation')}</Link>
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Shield className="h-4 w-4" />
              {t('insights.security')}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm space-y-2">
            <p>{t('insights.recentEvents')}: <strong>{security?.events?.length ?? '—'}</strong></p>
            <p className="text-muted-foreground">{t('insights.securityHint')}</p>
            <p>{t('insights.tesBackend')}: <code className="text-xs bg-muted px-1">{config?.compute?.tes_backend ?? '—'}</code></p>
            <Button asChild size="sm" variant="outline">
              <Link to={'/settings' as any}>{t('insights.openSecurity')}</Link>
            </Button>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <BarChart3 className="h-4 w-4" />
            {t('insights.platformHealth')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ServiceHealthPanel />
        </CardContent>
      </Card>
    </div>
  );
}
