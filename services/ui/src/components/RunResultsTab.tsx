import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { apiGet } from '@/api/client';
import type { ProvenanceGraphResponse } from '@/api/types';
import { useI18n } from '@/i18n/I18nProvider';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Download, Eye, Package } from 'lucide-react';
import { useAuthStore } from '@/stores/auth';

type OutputCategory = 'result' | 'artifact' | 'log';

interface OutputFile {
  file_id: string;
  name?: string;
  size?: number;
  location?: string;
  category?: OutputCategory;
  path?: string;
}

interface RunOutputs {
  output_files?: OutputFile[];
  artifact_files?: OutputFile[];
  log_files?: OutputFile[];
}

interface RunDetail {
  run_id: string;
  state: string;
  outputs?: RunOutputs | Record<string, unknown>;
}

interface ListedFile {
  id: string;
  name: string;
  size?: number;
  source: 'drs' | 'workdir';
  category: OutputCategory;
  path?: string;
}

function drsStreamUrl(objectId: string): string {
  return `/ga4gh/drs/v1/objects/${encodeURIComponent(objectId)}/stream`;
}

function workdirFileUrl(runId: string, fileId: string, inline = false): string {
  const base = `/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/outputs/files/${encodeURIComponent(fileId)}`;
  return inline ? `${base}?inline=true` : base;
}

function isPreviewable(name: string): boolean {
  const n = name.toLowerCase();
  return (
    n.endsWith('.txt') ||
    n.endsWith('.log') ||
    n.endsWith('.json') ||
    n.endsWith('.html') ||
    n.endsWith('.csv') ||
    n.endsWith('.tsv') ||
    n.endsWith('.vcf') ||
    n.endsWith('.cwl') ||
    n.endsWith('.wdl') ||
    n.endsWith('.nf')
  );
}

function inferCategory(name: string, relPath?: string): OutputCategory {
  const rel = relPath ?? name;
  if (
    name === 'stdout.txt' ||
    name === 'stderr.txt' ||
    name.endsWith('.log') ||
    name.startsWith('.command.') ||
    name === '.exitcode'
  ) {
    return 'log';
  }
  if (
    rel.startsWith('.nextflow/') ||
    rel.startsWith('.snakemake/') ||
    name === 'workflow.nf' ||
    name === 'nextflow.config' ||
    name === 'workflow.cwl' ||
    name === 'Snakefile' ||
    name === 'inputs.json' ||
    name === 'params.json' ||
    name === 'state.json' ||
    name.startsWith('MANIFEST-') ||
    name === 'CURRENT' ||
    name === 'LOCK' ||
    name === 'history' ||
    name.startsWith('index.')
  ) {
    return 'artifact';
  }
  return 'result';
}

async function fetchWithAuth(path: string): Promise<Response> {
  const jwt = useAuthStore.getState().passportJwt;
  return fetch(path, {
    headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
  });
}

async function downloadWithAuth(path: string, filename: string) {
  const res = await fetchWithAuth(path);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function collectWorkdirFiles(outputs: RunOutputs): ListedFile[] {
  const hasPartitions =
    (outputs.artifact_files?.length ?? 0) > 0 || (outputs.log_files?.length ?? 0) > 0;

  const push = (f: OutputFile, category: OutputCategory) => {
    files.push({
      id: f.file_id,
      name: f.name ?? f.file_id,
      size: f.size,
      source: 'workdir',
      category: f.category ?? category,
      path: f.path,
    });
  };

  const files: ListedFile[] = [];
  for (const f of outputs.output_files ?? []) {
    push(f, hasPartitions ? 'result' : inferCategory(f.name ?? f.file_id, f.path));
  }
  for (const f of outputs.artifact_files ?? []) {
    push(f, 'artifact');
  }
  for (const f of outputs.log_files ?? []) {
    push(f, 'log');
  }
  return files;
}

function FileTable({
  files,
  runId,
  onPreview,
  t,
}: {
  files: ListedFile[];
  runId: string;
  onPreview: (f: ListedFile) => void;
  t: (key: string) => string;
}) {
  if (files.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('run.noResults')}</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border">
            <th className="py-2 text-left font-medium">{t('run.colFile')}</th>
            <th className="py-2 text-left font-medium">{t('run.colSize')}</th>
            <th className="py-2 text-left font-medium">{t('run.colSource')}</th>
            <th className="py-2 text-left font-medium" />
          </tr>
        </thead>
        <tbody>
          {files.map((f) => (
            <tr key={`${f.source}-${f.id}`} className="border-b border-border/50">
              <td className="py-2">
                {f.source === 'drs' ? (
                  <Link to={`/data/objects/${f.id}` as any} className="text-primary hover:underline">
                    {f.name}
                  </Link>
                ) : (
                  <span className="font-mono text-xs sm:text-sm">{f.path ?? f.name}</span>
                )}
              </td>
              <td className="py-2 text-muted-foreground">
                {f.size != null ? `${(f.size / 1024).toFixed(1)} KB` : '—'}
              </td>
              <td className="py-2 text-muted-foreground">
                {f.source === 'drs' ? t('run.drsObject') : t('run.workDir')}
              </td>
              <td className="py-2">
                <div className="flex gap-1">
                  {f.source === 'workdir' && isPreviewable(f.name) && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="gap-1 h-8"
                      onClick={() => onPreview(f)}
                    >
                      <Eye className="h-3.5 w-3.5" />
                      {t('run.preview')}
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className="gap-1 h-8"
                    onClick={() =>
                      void downloadWithAuth(
                        f.source === 'drs'
                          ? drsStreamUrl(f.id)
                          : workdirFileUrl(runId, f.id),
                        f.name,
                      )
                    }
                  >
                    <Download className="h-3.5 w-3.5" />
                    {t('common.download')}
                  </Button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function RunResultsTab({ runId }: { runId: string }) {
  const { t } = useI18n();
  const [previewFile, setPreviewFile] = useState<ListedFile | null>(null);

  const { data: run } = useQuery({
    queryKey: ['wes', 'run', runId, 'outputs'],
    queryFn: () => apiGet<RunDetail>(`/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}`),
  });

  const { data: prov } = useQuery({
    queryKey: ['wes', 'provenance', runId, 'results'],
    queryFn: () =>
      apiGet<ProvenanceGraphResponse>(`/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/provenance`),
  });

  const { data: previewText, isLoading: previewLoading } = useQuery({
    queryKey: ['wes', 'run', runId, 'preview', previewFile?.id],
    enabled: !!previewFile && previewFile.source === 'workdir',
    queryFn: async () => {
      const res = await fetchWithAuth(workdirFileUrl(runId, previewFile!.id, true));
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const text = await res.text();
      const max = 256_000;
      return text.length > max ? `${text.slice(0, max)}\n\n… (${t('run.previewTruncated')})` : text;
    },
  });

  const allFiles: ListedFile[] = [];

  const rawOutputs = run?.outputs;
  if (rawOutputs && typeof rawOutputs === 'object' && 'output_files' in rawOutputs) {
    allFiles.push(...collectWorkdirFiles(rawOutputs as RunOutputs));
  }

  if (prov?.graph?.nodes) {
    const outputIds = new Set(
      prov.graph.edges
        .filter((e) => e.edge_type === 'output' || e.to_type === 'drs_object')
        .map((e) => (e.to_type === 'drs_object' ? e.to_id : e.from_id)),
    );
    for (const n of prov.graph.nodes) {
      if (n.type === 'drs_object' && outputIds.has(n.id)) {
        if (!allFiles.some((f) => f.id === n.id)) {
          allFiles.push({
            id: n.id,
            name: n.name ?? n.id,
            size: n.size,
            source: 'drs',
            category: 'result',
          });
        }
      }
    }
  }

  const resultFiles = allFiles.filter((f) => f.category === 'result');
  const secondaryFiles = allFiles.filter((f) => f.category !== 'result');

  const handleRoCrate = () => {
    void downloadWithAuth(
      `/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/export/ro-crate`,
      `run-${runId}.ro-crate.zip`,
    );
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-4">
          <div>
            <CardTitle>{t('run.resultsTitle')}</CardTitle>
            <p className="text-sm text-muted-foreground">{t('run.resultsHint')}</p>
          </div>
          <Button variant="outline" size="sm" className="gap-2 shrink-0" onClick={handleRoCrate}>
            <Package className="h-4 w-4" />
            {t('run.roCrate')}
          </Button>
        </CardHeader>
        <CardContent>
          {runId === 'demo-run-seed-01' && resultFiles.length > 0 && (
            <p className="mb-3 text-xs text-muted-foreground">{t('run.demoOutputsNote')}</p>
          )}

          <Tabs defaultValue="results">
            <TabsList>
              <TabsTrigger value="results">
                {t('run.tabResults')} ({resultFiles.length})
              </TabsTrigger>
              <TabsTrigger value="secondary">
                {t('run.tabLogsArtifacts')} ({secondaryFiles.length})
              </TabsTrigger>
            </TabsList>
            <TabsContent value="results">
              <FileTable files={resultFiles} runId={runId} onPreview={setPreviewFile} t={t} />
            </TabsContent>
            <TabsContent value="secondary">
              <p className="mb-3 text-xs text-muted-foreground">{t('run.artifactsHint')}</p>
              <FileTable files={secondaryFiles} runId={runId} onPreview={setPreviewFile} t={t} />
            </TabsContent>
          </Tabs>

          {previewFile && (
            <div className="mt-4 rounded-md border border-border bg-muted/30 p-3">
              <div className="mb-2 flex items-center justify-between gap-2">
                <p className="text-sm font-medium font-mono truncate">
                  {previewFile.path ?? previewFile.name}
                </p>
                <Button variant="ghost" size="sm" onClick={() => setPreviewFile(null)}>
                  {t('common.dismiss')}
                </Button>
              </div>
              {previewLoading ? (
                <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
              ) : (
                <pre className="max-h-96 overflow-auto text-xs whitespace-pre-wrap break-all">
                  {previewText ?? ''}
                </pre>
              )}
            </div>
          )}

          <p className="mt-3 text-xs text-muted-foreground">{t('run.roCrateHint')}</p>
        </CardContent>
      </Card>
    </div>
  );
}
