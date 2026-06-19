import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { apiGet } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import { WORKFLOW_ENGINES } from '@/lib/workflowEngines';
import {
  DOCKSTORE_TRS_BASE,
  descriptorTypeToEngineId,
  dockstoreToolclassToFerrum,
  type RegisterToolPreset,
} from '@/lib/trsCatalogs';
import { Download, ExternalLink, Loader2, Search } from 'lucide-react';

interface DockstoreToolVersion {
  id?: string;
  name?: string;
  descriptor_type?: string[];
}

interface DockstoreTool {
  id: string;
  name?: string;
  description?: string;
  organization?: string;
  toolclass?: { name?: string };
  versions?: DockstoreToolVersion[];
}

interface DescriptorResolveResponse {
  workflow_url?: string | null;
  content?: string | null;
  descriptor_url: string;
}

interface DockstoreSearchPanelProps {
  id?: string;
  onImport: (preset: RegisterToolPreset) => void;
}

function normalizeTools(payload: unknown): DockstoreTool[] {
  if (Array.isArray(payload)) return payload as DockstoreTool[];
  if (payload && typeof payload === 'object' && Array.isArray((payload as { tools?: unknown }).tools)) {
    return (payload as { tools: DockstoreTool[] }).tools;
  }
  return [];
}

function pickVersion(tool: DockstoreTool): DockstoreToolVersion | undefined {
  return tool.versions?.[0];
}

function pickDescriptorType(version?: DockstoreToolVersion): string | undefined {
  return version?.descriptor_type?.[0];
}

export function DockstoreSearchPanel({ id, onImport }: DockstoreSearchPanelProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState('');
  const [submittedQuery, setSubmittedQuery] = useState('');
  const [toolClass, setToolClass] = useState('all');
  const [engine, setEngine] = useState('all');
  const [importingId, setImportingId] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const { data, isFetching, error } = useQuery({
    queryKey: ['dockstore', 'search', submittedQuery, toolClass, engine],
    queryFn: async () => {
      const params = new URLSearchParams({
        trs_base_url: DOCKSTORE_TRS_BASE,
        limit: '20',
      });
      if (submittedQuery.trim()) params.set('q', submittedQuery.trim());
      if (toolClass === 'workflow') params.set('tool_class', 'Workflow');
      if (engine !== 'all') {
        const eng = WORKFLOW_ENGINES.find((e) => e.id === engine);
        if (eng) params.set('descriptor_type', eng.trsDescriptor);
      }
      const res = await apiGet<{ tools: unknown }>(`/admin/federation/proxy/trs/tools?${params}`);
      return normalizeTools(res.tools);
    },
    enabled: submittedQuery.trim().length >= 2,
    retry: false,
  });

  const tools = data ?? [];

  async function importTool(tool: DockstoreTool) {
    const version = pickVersion(tool);
    const descriptorType = pickDescriptorType(version);
    const versionName = version?.name ?? version?.id;
    if (!versionName || !descriptorType) {
      setImportError(t('tools.dockstoreImportFailed'));
      return;
    }
    setImportingId(tool.id);
    setImportError(null);
    try {
      const params = new URLSearchParams({
        trs_base_url: DOCKSTORE_TRS_BASE,
        tool_id: tool.id,
        version_id: versionName,
        descriptor_type: descriptorType,
      });
      const resolved = await apiGet<DescriptorResolveResponse>(
        `/admin/federation/proxy/trs/descriptor?${params}`,
      );
      const engineId = descriptorTypeToEngineId(descriptorType);
      onImport({
        name: tool.name ?? tool.id,
        description: tool.description?.slice(0, 240) ?? tool.organization,
        workflowUrl: resolved.workflow_url ?? resolved.descriptor_url,
        workflowContent: resolved.content ?? undefined,
        engineId,
        toolclass: dockstoreToolclassToFerrum(tool.toolclass?.name),
      });
    } catch (e) {
      setImportError(e instanceof Error ? e.message : t('tools.dockstoreImportFailed'));
    } finally {
      setImportingId(null);
    }
  }

  return (
    <Card id={id}>
      <CardHeader>
        <CardTitle>{t('tools.dockstoreTitle')}</CardTitle>
        <p className="text-sm text-muted-foreground">{t('tools.dockstoreHint')}</p>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 md:grid-cols-4">
          <div className="md:col-span-2 space-y-2">
            <Label htmlFor="dockstore-q">{t('tools.dockstoreSearchLabel')}</Label>
            <Input
              id="dockstore-q"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('tools.dockstoreSearchPlaceholder')}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && query.trim().length >= 2) setSubmittedQuery(query.trim());
              }}
            />
          </div>
          <div className="space-y-2">
            <Label>{t('tools.toolclassLabel')}</Label>
            <Select value={toolClass} onValueChange={setToolClass}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{t('tools.dockstoreClassAll')}</SelectItem>
                <SelectItem value="workflow">{t('tools.dockstoreClassWorkflow')}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label>{t('tools.engineLabel')}</Label>
            <Select value={engine} onValueChange={setEngine}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{t('tools.dockstoreEngineAll')}</SelectItem>
                {WORKFLOW_ENGINES.map((e) => (
                  <SelectItem key={e.id} value={e.id}>
                    {t(e.labelKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
        <Button
          type="button"
          className="gap-2"
          disabled={query.trim().length < 2 || isFetching}
          onClick={() => setSubmittedQuery(query.trim())}
        >
          {isFetching ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
          {t('tools.dockstoreSearch')}
        </Button>

        {error && (
          <p className="text-sm text-destructive">
            {error instanceof Error ? error.message : t('tools.dockstoreImportFailed')}
          </p>
        )}
        {importError && <p className="text-sm text-destructive">{importError}</p>}

        {submittedQuery && !isFetching && tools.length === 0 && (
          <p className="text-sm text-muted-foreground">{t('tools.dockstoreNoResults')}</p>
        )}

        {tools.length > 0 && (
          <ul className="space-y-3">
            {tools.map((tool) => {
              const version = pickVersion(tool);
              const descriptorType = pickDescriptorType(version);
              return (
                <li key={tool.id} className="rounded-lg border border-border p-4">
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="min-w-0">
                      <p className="font-medium">{tool.name ?? tool.id}</p>
                      {tool.description && (
                        <p className="text-sm text-muted-foreground mt-1 line-clamp-2">
                          {tool.description}
                        </p>
                      )}
                      <p className="text-xs text-muted-foreground mt-2">
                        {tool.toolclass?.name && <span>{tool.toolclass.name}</span>}
                        {descriptorType && <span> · {descriptorType}</span>}
                        {version?.name && <span> · v{version.name}</span>}
                        {tool.organization && <span> · {tool.organization}</span>}
                      </p>
                    </div>
                    <div className="flex shrink-0 flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="secondary"
                        className="gap-1"
                        disabled={importingId === tool.id || !descriptorType}
                        onClick={() => void importTool(tool)}
                      >
                        {importingId === tool.id ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <Download className="h-3.5 w-3.5" />
                        )}
                        {importingId === tool.id
                          ? t('tools.dockstoreImporting')
                          : t('tools.dockstoreImport')}
                      </Button>
                      <Button asChild size="sm" variant="outline" className="gap-1">
                        <a
                          href={`https://dockstore.org/search?search=${encodeURIComponent(tool.name ?? tool.id)}`}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          <ExternalLink className="h-3.5 w-3.5" />
                          {t('tools.dockstoreOpen')}
                        </a>
                      </Button>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
