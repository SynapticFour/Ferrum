import { useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { apiGet } from '@/api/client';
import { loadFederationPrefs } from '@/stores/federation';
import { Wrench, AlertCircle, Play, BookOpen, ExternalLink, Download } from 'lucide-react';
import { RegisterToolDialog } from '@/components/RegisterToolDialog';
import { DockstoreSearchPanel } from '@/components/DockstoreSearchPanel';
import { useI18n } from '@/i18n/I18nProvider';
import { WORKFLOW_ENGINES } from '@/lib/workflowEngines';
import {
  TRS_EXTERNAL_CATALOGS,
  TRS_IMPORT_PRESETS,
  type RegisterToolPreset,
} from '@/lib/trsCatalogs';

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
            {t.toolclass?.name && ` · ${t.toolclass.name}`}
          </p>
        </li>
      ))}
    </ul>
  );
}

export function ToolRegistry() {
  const { t } = useI18n();
  const [tab, setTab] = useState('local');
  const [registerPreset, setRegisterPreset] = useState<RegisterToolPreset | null>(null);
  const prefs = loadFederationPrefs();

  const { data, isLoading, error } = useQuery({
    queryKey: ['trs', 'tools', 'local'],
    queryFn: () => apiGet<ToolListResponse>('/ga4gh/trs/v2/tools'),
    retry: false,
  });

  const { data: federationStatus } = useQuery({
    queryKey: ['admin', 'federation', 'status'],
    queryFn: () => apiGet<{ service_registry_url?: string }>('/admin/federation/status'),
    retry: false,
  });

  const registryUrl = prefs.registryUrl || federationStatus?.service_registry_url || '';

  const { data: registryServices } = useQuery({
    queryKey: ['federation', 'registry', registryUrl],
    queryFn: () =>
      fetch('/admin/federation/registry/services', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ registry_url: registryUrl }),
      }).then((r) => r.json()) as Promise<{ services: RegisteredService[] }>,
    enabled: tab === 'federation' && !!registryUrl,
    retry: false,
  });

  const trsServices =
    registryServices?.services.filter((s) => s.type.artifact === 'tool-registry') ?? [];
  const remoteTrs = trsServices[0]?.url;

  const { data: remoteTools, isLoading: remoteLoading } = useQuery({
    queryKey: ['trs', 'remote', remoteTrs],
    queryFn: () =>
      apiGet<{ trs_base_url?: string; tools: Tool[] | { tools?: Tool[] } }>(
        `/admin/federation/proxy/trs/tools?trs_base_url=${encodeURIComponent(remoteTrs!)}`,
      ).then((r) => {
        const tools = r.tools;
        if (Array.isArray(tools)) return tools;
        if (tools && typeof tools === 'object' && Array.isArray(tools.tools)) return tools.tools;
        return [];
      }),
    enabled: tab === 'federation' && !!remoteTrs,
    retry: false,
  });

  const localTools = Array.isArray(data) ? data : (data?.tools ?? []);
  const federatedTools = Array.isArray(remoteTools) ? remoteTools : [];

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t('tools.title')}</h1>
          <p className="text-muted-foreground">{t('tools.subtitle')}</p>
        </div>
        <RegisterToolDialog
          preset={registerPreset}
          onPresetApplied={() => setRegisterPreset(null)}
        />
      </div>

      <Card className="border-primary/20 bg-primary/5">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <BookOpen className="h-4 w-4" />
            {t('tools.guideTitle')}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-muted-foreground">
          <p>{t('tools.guideBody')}</p>
          <div className="flex flex-wrap gap-2">
            {WORKFLOW_ENGINES.map((e) => (
              <span key={e.id} className="rounded-full border border-border bg-background px-2 py-0.5 text-xs">
                {t(e.labelKey)}
              </span>
            ))}
          </div>
          <div className="flex flex-wrap gap-2 pt-1">
            <Button asChild size="sm" variant="outline" className="gap-1">
              <Link to={'/workflows' as any}>
                <Play className="h-3.5 w-3.5" />
                {t('tools.goRunAnalysis')}
              </Link>
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t('tools.catalogTitle')}</CardTitle>
          <p className="text-sm text-muted-foreground">{t('tools.catalogHint')}</p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            {TRS_EXTERNAL_CATALOGS.map((catalog) => (
              <div key={catalog.id} className="rounded-lg border border-border p-3">
                <p className="font-medium">{catalog.name}</p>
                <p className="text-xs text-muted-foreground mt-1">{t(catalog.descriptionKey)}</p>
                <Button asChild size="sm" variant="outline" className="mt-2 gap-1">
                  <a href={catalog.url} target="_blank" rel="noopener noreferrer">
                    <ExternalLink className="h-3.5 w-3.5" />
                    {t('tools.browseCatalog')}
                  </a>
                </Button>
              </div>
            ))}
          </div>
          <div className="border-t border-border/60 pt-3">
            <p className="text-xs font-medium text-muted-foreground mb-2">{t('tools.importPreset')}</p>
            <div className="flex flex-wrap gap-2">
              {TRS_IMPORT_PRESETS.map((preset) => (
                <Button
                  key={preset.id}
                  size="sm"
                  variant="secondary"
                  className="gap-1"
                  onClick={() =>
                    setRegisterPreset({
                      name: t(preset.nameKey),
                      workflowUrl: preset.workflowUrl,
                      engineId: preset.engineId,
                      toolclass: preset.toolclass,
                      description: t(preset.sourceKey),
                    })
                  }
                >
                  <Download className="h-3.5 w-3.5" />
                  {t(preset.nameKey)}
                </Button>
              ))}
            </div>
          </div>
        </CardContent>
      </Card>

      <DockstoreSearchPanel onImport={setRegisterPreset} />

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="local">{t('tools.tabLocal')}</TabsTrigger>
          <TabsTrigger value="federation">{t('tools.tabFederation')}</TabsTrigger>
        </TabsList>
        <TabsContent value="local">
          {error && (
            <div className="flex items-center gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400 mb-4">
              <AlertCircle className="h-4 w-4 shrink-0" />
              {t('tools.trsUnavailable')}
            </div>
          )}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Wrench className="h-4 w-4" />
                {t('tools.localListTitle')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              {isLoading && <p className="text-muted-foreground text-sm">{t('common.loading')}</p>}
              <ToolList tools={localTools} empty={t('tools.localEmpty')} />
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="federation">
          <Card>
            <CardHeader>
              <CardTitle>{t('tools.federationTitle')}</CardTitle>
              <p className="text-sm text-muted-foreground">{t('tools.federationHint', { url: prefs.registryUrl || '—' })}</p>
            </CardHeader>
            <CardContent>
              {!prefs.registryUrl && <p className="text-sm text-muted-foreground">{t('tools.federationNoRegistry')}</p>}
              {remoteLoading && <p className="text-sm text-muted-foreground">{t('common.loading')}</p>}
              <ToolList
                tools={federatedTools}
                empty={remoteTrs ? t('tools.federationEmpty') : t('tools.federationNoTrs')}
              />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
