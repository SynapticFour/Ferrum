import { useEffect, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { apiGet } from '@/api/client';
import { CURATED_WORKFLOWS, trsDescriptorUrl } from '@/lib/workflows';
import { engineByWesType } from '@/lib/workflowEngines';
import { useWdlDescriptor } from '@/hooks/useWdlDescriptor';
import { initParamValues, WorkflowParamForm } from '@/components/WorkflowParamForm';
import { useI18n } from '@/i18n/I18nProvider';
import type { DrsObject } from '@/api/types';
import {
  buildFlatWorkflowParams,
  resolvePerSampleFileParams,
  resolveSharedFileFormValues,
  resolveSharedFileParams,
  submitWorkflowRun,
} from '@/lib/wesSubmit';
import { isPerSampleFileInput } from '@/lib/wdlInputs';
import { drsStorageKind } from '@/lib/drsStorage';
import { Play, Loader2, Users } from 'lucide-react';

const COHORTS_BASE = '/cohorts/v1';

interface CohortSample {
  sample_id: string;
  drs_object_ids: string[];
}

interface RunCohortDialogProps {
  cohortId: string;
  cohortName: string;
  workspaceId?: string | null;
  disabled?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  hideTrigger?: boolean;
}

type SourceTab = 'curated' | 'trs';

export function RunCohortDialog({
  cohortId,
  cohortName,
  workspaceId,
  disabled,
  open: controlledOpen,
  onOpenChange,
  hideTrigger,
}: RunCohortDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [internalOpen, setInternalOpen] = useState(false);
  const open = controlledOpen ?? internalOpen;
  const setOpen = onOpenChange ?? setInternalOpen;

  const [sourceTab, setSourceTab] = useState<SourceTab>('curated');
  const [curatedId, setCuratedId] = useState(CURATED_WORKFLOWS[0]?.id ?? '');
  const [trsToolId, setTrsToolId] = useState('');
  const [workflowUrl, setWorkflowUrl] = useState(CURATED_WORKFLOWS[0]?.workflowUrl ?? '');
  const [workflowType, setWorkflowType] = useState('WDL');
  const [paramValues, setParamValues] = useState<Record<string, string>>({});
  const [fileLabels, setFileLabels] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);

  const { data: samplesResp } = useQuery({
    queryKey: ['cohort-samples', cohortId, 'run'],
    queryFn: () =>
      apiGet<{ samples: CohortSample[] }>(
        `${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId)}/samples?limit=500`,
      ),
    enabled: open,
  });
  const samples = samplesResp?.samples ?? [];

  const { data: drsObjects } = useQuery({
    queryKey: ['drs', 'objects', 'cohort-run'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    enabled: open,
  });
  const drsLookup = useMemo(() => {
    const m = new Map<string, DrsObject>();
    for (const o of Array.isArray(drsObjects) ? drsObjects : []) m.set(o.id, o);
    return m;
  }, [drsObjects]);

  const { data: workspaceObjects } = useQuery({
    queryKey: ['drs', 'workspace', workspaceId, 'cohort-run'],
    queryFn: () =>
      apiGet<DrsObject[]>(
        `/ga4gh/drs/v1/objects?workspace_id=${encodeURIComponent(workspaceId!)}&limit=200`,
      ),
    enabled: open && !!workspaceId,
  });

  const { data: trsTools } = useQuery({
    queryKey: ['trs', 'tools', 'cohort-run'],
    queryFn: () => apiGet<Array<{ id: string; name?: string; versions?: Array<{ id: string }> }>>('/ga4gh/trs/v2/tools'),
    enabled: open && sourceTab === 'trs',
    retry: false,
  });

  const { data: wdlParsed, isLoading: wdlLoading } = useWdlDescriptor(workflowUrl, workflowType, open);

  const urlBackedSamples = useMemo(() => {
    const hits: Array<{ sampleId: string; objectIds: string[] }> = [];
    for (const sample of samples) {
      const urlIds = (sample.drs_object_ids ?? []).filter((oid) => {
        const obj = drsLookup.get(oid);
        return obj && drsStorageKind(obj) === 'url';
      });
      if (urlIds.length) hits.push({ sampleId: sample.sample_id, objectIds: urlIds });
    }
    return hits;
  }, [samples, drsLookup]);

  const sharedAutoCount = useMemo(() => {
    if (!wdlParsed?.inputs.length || !workspaceObjects?.length) return 0;
    const fileInputs = wdlParsed.inputs.filter((i) => i.wdlType === 'File');
    return Object.keys(resolveSharedFileFormValues(fileInputs, workspaceObjects).values).length;
  }, [wdlParsed, workspaceObjects]);

  useEffect(() => {
    if (!wdlParsed?.inputs.length) return;
    const base = initParamValues(wdlParsed.inputs, true);
    const fileInputs = wdlParsed.inputs.filter((i) => i.wdlType === 'File');
    const ws = Array.isArray(workspaceObjects) ? workspaceObjects : [];
    const { values: shared, labels } = ws.length
      ? resolveSharedFileFormValues(fileInputs, ws)
      : { values: {} as Record<string, string>, labels: {} as Record<string, string> };
    setParamValues({ ...base, ...shared });
    setFileLabels(labels);
  }, [wdlParsed?.workflowName, wdlParsed?.inputs, workspaceObjects, workspaceId]);

  const applyCurated = (id: string) => {
    const wf = CURATED_WORKFLOWS.find((c) => c.id === id);
    if (!wf) return;
    setCuratedId(id);
    setWorkflowUrl(wf.workflowUrl);
    setWorkflowType(wf.workflowType);
  };

  const applyTrsTool = (toolId: string) => {
    setTrsToolId(toolId);
    const tools = Array.isArray(trsTools) ? trsTools : [];
    const tool = tools.find((x) => x.id === toolId);
    const version = tool?.versions?.[0];
    if (tool && version) {
      const desc = tool.id.includes('cwl') ? 'CWL' : tool.id.includes('snakemake') ? 'SMK' : tool.id.includes('nextflow') ? 'NFL' : 'WDL';
      setWorkflowUrl(trsDescriptorUrl(tool.id, version.id, desc));
      const eng = engineByWesType(desc === 'NFL' ? 'Nextflow' : desc === 'SMK' ? 'Snakemake' : desc);
      setWorkflowType(eng?.wesType ?? 'WDL');
    }
  };

  const runBatch = useMutation({
    mutationFn: async () => {
      if (!samples.length) throw new Error(t('cohort.runNoSamples'));
      const workflowName =
        wdlParsed?.workflowName ??
        CURATED_WORKFLOWS.find((c) => c.id === curatedId)?.paramPrefix ??
        'Workflow';
      const fileInputs = wdlParsed?.inputs.filter((i) => i.wdlType === 'File') ?? [];
      const sharedParams = buildFlatWorkflowParams(workflowName, paramValues);
      const ws = Array.isArray(workspaceObjects) ? workspaceObjects : [];
      const autoShared = ws.length
        ? resolveSharedFileParams(workflowName, fileInputs, ws)
        : {};
      const mergedShared = { ...autoShared, ...sharedParams };
      const runIds: string[] = [];

      const missingShared = fileInputs.filter(
        (inp) =>
          !isPerSampleFileInput(inp.name) &&
          !mergedShared[`${workflowName}.${inp.name}`],
      );
      if (missingShared.length) {
        throw new Error(
          t('cohort.runMissingShared', {
            fields: missingShared.map((m) => m.name).join(', '),
          }),
        );
      }

      for (let i = 0; i < samples.length; i++) {
        const sample = samples[i];
        setProgress(
          t('cohort.runProgress', {
            current: String(i + 1),
            total: String(samples.length),
            sample: sample.sample_id,
          }),
        );
        const perSample = resolvePerSampleFileParams(
          workflowName,
          fileInputs,
          sample.drs_object_ids ?? [],
          drsLookup,
        );
        const workflow_params = { ...mergedShared, ...perSample };
        const missing = fileInputs.filter(
          (inp) =>
            isPerSampleFileInput(inp.name) &&
            !workflow_params[`${workflowName}.${inp.name}`],
        );
        if (missing.length) {
          throw new Error(
            t('cohort.runMissingData', {
              sample: sample.sample_id,
              fields: missing.map((m) => m.name).join(', '),
            }),
          );
        }
        const res = await submitWorkflowRun({
          workflowType,
          workflowUrl,
          workflowParams: workflow_params,
          workspaceId,
          tags: {
            source: 'ferrum-run-on-cohort',
            cohort_id: cohortId,
            sample_id: sample.sample_id,
          },
        });
        if (res.run_id) runIds.push(res.run_id);
      }
      return runIds;
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      setProgress(null);
      void qc.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
    onError: (e: Error) => {
      setError(e.message);
      setProgress(null);
    },
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {!hideTrigger && (
        <DialogTrigger asChild>
          <Button disabled={disabled || samples.length === 0} className="gap-2">
            <Users className="h-4 w-4" />
            {t('cohort.runOnCohort')}
          </Button>
        </DialogTrigger>
      )}
      <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('cohort.runOnCohortTitle')}</DialogTitle>
          <p className="text-sm text-muted-foreground">
            {t('cohort.runOnCohortHint', { name: cohortName, count: String(samples.length) })}
          </p>
        </DialogHeader>

        <Tabs value={sourceTab} onValueChange={(v) => setSourceTab(v as SourceTab)}>
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="curated">{t('workflows.sourceCurated')}</TabsTrigger>
            <TabsTrigger value="trs">{t('workflows.sourceTrs')}</TabsTrigger>
          </TabsList>
          <TabsContent value="curated" className="pt-2">
            <Label>{t('workflows.workflowLabel')}</Label>
            <Select value={curatedId} onValueChange={applyCurated}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CURATED_WORKFLOWS.map((wf) => (
                  <SelectItem key={wf.id} value={wf.id}>
                    {t(wf.nameKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </TabsContent>
          <TabsContent value="trs" className="pt-2">
            <Label>{t('workflows.pickWorkflow')}</Label>
            <Select value={trsToolId} onValueChange={applyTrsTool}>
              <SelectTrigger>
                <SelectValue placeholder={t('workflows.pickWorkflow')} />
              </SelectTrigger>
              <SelectContent>
                {(Array.isArray(trsTools) ? trsTools : []).map((tool) => (
                  <SelectItem key={tool.id} value={tool.id}>
                    {tool.name ?? tool.id}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </TabsContent>
        </Tabs>

        {wdlLoading && <p className="text-sm text-muted-foreground">{t('workflows.loadingParams')}</p>}
        {wdlParsed && wdlParsed.inputs.length > 0 && (
          <WorkflowParamForm
            workflowName={wdlParsed.workflowName}
            inputs={wdlParsed.inputs}
            values={paramValues}
            onChange={setParamValues}
            hidePerSampleFiles
            fileLabels={fileLabels}
          />
        )}

        {sharedAutoCount > 0 && (
          <p className="text-xs text-muted-foreground">{t('cohort.runSharedAutoFilled', { count: String(sharedAutoCount) })}</p>
        )}

        {urlBackedSamples.length > 0 && (
          <div className="text-sm border rounded-md p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 space-y-1">
            <p className="font-medium">{t('cohort.runUrlBackedWarning')}</p>
            <p className="text-muted-foreground">{t('cohort.runUrlBackedHint')}</p>
            <ul className="list-disc list-inside text-xs text-muted-foreground">
              {urlBackedSamples.map((s) => (
                <li key={s.sampleId}>
                  {s.sampleId}: {s.objectIds.join(', ')}
                </li>
              ))}
            </ul>
          </div>
        )}

        {progress && <p className="text-sm text-muted-foreground">{progress}</p>}
        {error && <p className="text-sm text-destructive">{error}</p>}
        {samples.length === 0 && (
          <p className="text-sm text-amber-700 dark:text-amber-300 border rounded-md p-3 bg-amber-500/10">
            {t('cohort.runNoSamples')}
          </p>
        )}
        <Button
          type="button"
          className="w-full gap-2"
          disabled={runBatch.isPending || !workflowUrl || samples.length === 0}
          onClick={() => runBatch.mutate()}
        >
          {runBatch.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
          {t('cohort.runOnCohortConfirm', { count: String(samples.length) })}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
