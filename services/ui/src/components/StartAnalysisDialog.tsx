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
import { DRSObjectPicker } from '@/components/DRSObjectPicker';
import { useI18n } from '@/i18n/I18nProvider';
import type { DrsObject } from '@/api/types';
import {
  buildFlatWorkflowParams,
  drsStreamUrl,
  resolvePerSampleFileParams,
  submitWorkflowRun,
} from '@/lib/wesSubmit';
import { isPerSampleFileInput } from '@/lib/wdlInputs';
import { ArrowLeft, ArrowRight, Loader2, Play, Users } from 'lucide-react';

const COHORTS_BASE = '/cohorts/v1';

interface CohortSample {
  sample_id: string;
  drs_object_ids: string[];
}

interface CohortSummary {
  id: string;
  name: string;
  sample_count: number;
}

type SourceTab = 'curated' | 'trs';
type InputMode = 'single' | 'cohort';

export interface StartAnalysisDialogProps {
  workspaceId: string;
  defaultDrsObjectId?: string;
  defaultCohortId?: string;
  disabled?: boolean;
  /** Open the wizard on mount (e.g. from ?analyze=1 deep link). */
  autoOpen?: boolean;
  /** Wizard step when opened; defaults to 2 when a DRS object is pre-selected. */
  initialStep?: number;
  triggerLabelKey?: string;
}

export function StartAnalysisDialog({
  workspaceId,
  defaultDrsObjectId,
  defaultCohortId,
  disabled,
  autoOpen,
  initialStep,
  triggerLabelKey = 'workspace.startAnalysis',
}: StartAnalysisDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(1);
  const [sourceTab, setSourceTab] = useState<SourceTab>('curated');
  const [curatedId, setCuratedId] = useState(CURATED_WORKFLOWS[0]?.id ?? '');
  const [trsToolId, setTrsToolId] = useState('');
  const [workflowUrl, setWorkflowUrl] = useState(CURATED_WORKFLOWS[0]?.workflowUrl ?? '');
  const [workflowType, setWorkflowType] = useState('WDL');
  const [inputMode, setInputMode] = useState<InputMode>(defaultCohortId ? 'cohort' : 'single');
  const [cohortId, setCohortId] = useState(defaultCohortId ?? '');
  const [selectedObject, setSelectedObject] = useState<DrsObject | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [paramValues, setParamValues] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);

  const { data: trsTools } = useQuery({
    queryKey: ['trs', 'tools', 'start-analysis'],
    queryFn: () => apiGet<Array<{ id: string; name?: string; versions?: Array<{ id: string }> }>>('/ga4gh/trs/v2/tools'),
    enabled: open && sourceTab === 'trs',
    retry: false,
  });

  const { data: cohortsResp } = useQuery({
    queryKey: ['cohorts', 'workspace', workspaceId, 'analysis-wizard'],
    queryFn: () =>
      apiGet<{ cohorts: CohortSummary[] }>(
        `${COHORTS_BASE}/cohorts?workspace_id=${encodeURIComponent(workspaceId)}&limit=50`,
      ),
    enabled: open && inputMode === 'cohort',
  });
  const cohorts = cohortsResp?.cohorts ?? [];

  const { data: samplesResp } = useQuery({
    queryKey: ['cohort-samples', cohortId, 'analysis-wizard'],
    queryFn: () =>
      apiGet<{ samples: CohortSample[] }>(
        `${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId)}/samples?limit=500`,
      ),
    enabled: open && inputMode === 'cohort' && !!cohortId,
  });
  const samples = samplesResp?.samples ?? [];

  const { data: defaultObject } = useQuery({
    queryKey: ['drs', 'object', defaultDrsObjectId, 'start-analysis'],
    queryFn: () =>
      apiGet<DrsObject>(`/ga4gh/drs/v1/objects/${encodeURIComponent(defaultDrsObjectId!)}`),
    enabled: !!defaultDrsObjectId,
  });

  const { data: drsObjects } = useQuery({
    queryKey: ['drs', 'objects', 'analysis-wizard'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    enabled: open,
  });
  const drsLookup = useMemo(() => {
    const m = new Map<string, DrsObject>();
    for (const o of Array.isArray(drsObjects) ? drsObjects : []) m.set(o.id, o);
    return m;
  }, [drsObjects]);

  const { data: wdlParsed, isLoading: wdlLoading } = useWdlDescriptor(workflowUrl, workflowType, open);

  useEffect(() => {
    if (autoOpen) setOpen(true);
  }, [autoOpen]);

  useEffect(() => {
    if (!open) return;
    setStep(initialStep ?? (defaultDrsObjectId ? 2 : 1));
    setError(null);
    setProgress(null);
    if (defaultCohortId) {
      setInputMode('cohort');
      setCohortId(defaultCohortId);
    } else if (defaultDrsObjectId) {
      setInputMode('single');
    }
  }, [open, defaultCohortId, defaultDrsObjectId, initialStep]);

  useEffect(() => {
    if (defaultObject) setSelectedObject(defaultObject);
  }, [defaultObject]);

  useEffect(() => {
    if (!defaultDrsObjectId || !drsLookup.size) return;
    const obj = drsLookup.get(defaultDrsObjectId);
    if (obj) setSelectedObject(obj);
  }, [defaultDrsObjectId, drsLookup]);

  useEffect(() => {
    if (wdlParsed?.inputs.length) {
      setParamValues(initParamValues(wdlParsed.inputs, inputMode === 'cohort'));
    }
  }, [wdlParsed?.workflowName, wdlParsed?.inputs.length, inputMode]);

  useEffect(() => {
    if (!selectedObject || !wdlParsed?.inputs.length) return;
    const fileInput = wdlParsed.inputs.find((i) => i.wdlType === 'File' && !isPerSampleFileInput(i.name));
    if (!fileInput) return;
    setParamValues((prev) => ({
      ...prev,
      [fileInput.name]: drsStreamUrl(selectedObject.id),
    }));
  }, [selectedObject?.id, wdlParsed?.workflowName, wdlParsed?.inputs]);

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

  const runAnalysis = useMutation({
    mutationFn: async () => {
      const workflowName =
        wdlParsed?.workflowName ??
        CURATED_WORKFLOWS.find((c) => c.id === curatedId)?.paramPrefix ??
        'Workflow';
      const sharedParams = buildFlatWorkflowParams(workflowName, paramValues);

      if (inputMode === 'cohort') {
        if (!cohortId) throw new Error(t('analysisWizard.noCohortSelected'));
        if (!samples.length) throw new Error(t('cohort.runNoSamples'));
        const fileInputs = wdlParsed?.inputs.filter((i) => i.wdlType === 'File') ?? [];
        const runIds: string[] = [];
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
          const workflow_params = { ...sharedParams, ...perSample };
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
              source: 'ferrum-start-analysis',
              cohort_id: cohortId,
              sample_id: sample.sample_id,
            },
          });
          if (res.run_id) runIds.push(res.run_id);
        }
        return { mode: 'cohort' as const, count: runIds.length };
      }

      const workflow_params =
        wdlParsed && wdlParsed.inputs.length > 0 ? sharedParams : {};
      const res = await submitWorkflowRun({
        workflowType,
        workflowUrl,
        workflowParams: workflow_params,
        workspaceId,
        tags: { source: 'ferrum-start-analysis' },
      });
      return { mode: 'single' as const, runId: res.run_id };
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

  const workflowLabel =
    sourceTab === 'curated'
      ? t(CURATED_WORKFLOWS.find((c) => c.id === curatedId)?.nameKey ?? 'workflows.workflowLabel')
      : (Array.isArray(trsTools) ? trsTools : []).find((x) => x.id === trsToolId)?.name ?? trsToolId;

  const canAdvanceStep1 = workflowUrl.trim().length > 0;
  const canAdvanceStep2 =
    inputMode === 'single' ? !!selectedObject || !wdlParsed?.inputs.some((i) => i.wdlType === 'File') : !!cohortId && samples.length > 0;

  return (
    <>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger asChild>
          <Button disabled={disabled} className="gap-2">
            <Play className="h-4 w-4" />
            {t(triggerLabelKey)}
          </Button>
        </DialogTrigger>
        <DialogContent className="max-w-xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t('analysisWizard.title')}</DialogTitle>
            <p className="text-sm text-muted-foreground">{t('analysisWizard.subtitle')}</p>
          </DialogHeader>

          <ol className="flex gap-2 text-xs">
            {[1, 2, 3].map((n) => (
              <li
                key={n}
                className={`rounded-full px-2 py-1 border ${
                  n === step ? 'border-primary bg-primary/10 text-primary' : 'border-border text-muted-foreground'
                }`}
              >
                {n}. {t(`analysisWizard.step${n}` as 'analysisWizard.step1')}
              </li>
            ))}
          </ol>

          {step === 1 && (
            <div className="space-y-4">
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
            </div>
          )}

          {step === 2 && (
            <div className="space-y-4">
              <Tabs value={inputMode} onValueChange={(v) => setInputMode(v as InputMode)}>
                <TabsList className="grid w-full grid-cols-2">
                  <TabsTrigger value="single">{t('analysisWizard.inputSingle')}</TabsTrigger>
                  <TabsTrigger value="cohort">{t('analysisWizard.inputCohort')}</TabsTrigger>
                </TabsList>
                <TabsContent value="single" className="space-y-3 pt-2">
                  <p className="text-sm text-muted-foreground">{t('analysisWizard.inputSingleHint')}</p>
                  {selectedObject ? (
                    <div className="rounded-md border p-3 text-sm">
                      <p className="font-medium">{selectedObject.name ?? selectedObject.id}</p>
                      <p className="text-xs text-muted-foreground font-mono">{selectedObject.id}</p>
                    </div>
                  ) : (
                    <p className="text-sm text-muted-foreground">{t('analysisWizard.noDataSelected')}</p>
                  )}
                  <Button type="button" variant="outline" onClick={() => setPickerOpen(true)}>
                    {t('workflows.pickData')}
                  </Button>
                </TabsContent>
                <TabsContent value="cohort" className="space-y-3 pt-2">
                  <p className="text-sm text-muted-foreground">{t('analysisWizard.inputCohortHint')}</p>
                  {cohorts.length === 0 ? (
                    <p className="text-sm text-amber-600 dark:text-amber-400">{t('analysisWizard.noCohorts')}</p>
                  ) : (
                    <>
                      <Label>{t('analysisWizard.selectCohort')}</Label>
                      <Select value={cohortId} onValueChange={setCohortId}>
                        <SelectTrigger>
                          <SelectValue placeholder={t('analysisWizard.selectCohort')} />
                        </SelectTrigger>
                        <SelectContent>
                          {cohorts.map((c) => (
                            <SelectItem key={c.id} value={c.id}>
                              {c.name} ({c.sample_count})
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      {cohortId && (
                        <p className="text-xs text-muted-foreground flex items-center gap-1">
                          <Users className="h-3.5 w-3.5" />
                          {t('cohort.runOnCohortHint', {
                            name: cohorts.find((c) => c.id === cohortId)?.name ?? cohortId,
                            count: String(samples.length),
                          })}
                        </p>
                      )}
                    </>
                  )}
                </TabsContent>
              </Tabs>
            </div>
          )}

          {step === 3 && (
            <div className="space-y-4">
              <div className="rounded-md border bg-muted/30 p-3 text-sm space-y-1">
                <p>
                  <span className="text-muted-foreground">{t('workflows.workflowLabel')}:</span> {workflowLabel}
                </p>
                <p>
                  <span className="text-muted-foreground">{t('analysisWizard.inputLabel')}:</span>{' '}
                  {inputMode === 'cohort'
                    ? `${cohorts.find((c) => c.id === cohortId)?.name ?? cohortId} (${samples.length} ${t('analysisWizard.samples')})`
                    : selectedObject?.name ?? selectedObject?.id ?? '—'}
                </p>
              </div>
              {wdlLoading && <p className="text-sm text-muted-foreground">{t('workflows.loadingParams')}</p>}
              {wdlParsed && wdlParsed.inputs.length > 0 && (
                <WorkflowParamForm
                  workflowName={wdlParsed.workflowName}
                  inputs={wdlParsed.inputs}
                  values={paramValues}
                  onChange={setParamValues}
                  hidePerSampleFiles={inputMode === 'cohort'}
                />
              )}
              {progress && <p className="text-sm text-muted-foreground">{progress}</p>}
              {error && <p className="text-sm text-destructive">{error}</p>}
              <Button
                type="button"
                className="w-full gap-2"
                disabled={runAnalysis.isPending || !workflowUrl}
                onClick={() => runAnalysis.mutate()}
              >
                {runAnalysis.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Play className="h-4 w-4" />
                )}
                {inputMode === 'cohort'
                  ? t('cohort.runOnCohortConfirm', { count: String(samples.length) })
                  : t('workflows.run')}
              </Button>
            </div>
          )}

          <div className="flex justify-between pt-2">
            <Button
              type="button"
              variant="outline"
              className="gap-2"
              disabled={step <= 1 || runAnalysis.isPending}
              onClick={() => setStep((s) => Math.max(1, s - 1))}
            >
              <ArrowLeft className="h-4 w-4" />
              {t('study.back')}
            </Button>
            {step < 3 && (
              <Button
                type="button"
                className="gap-2"
                disabled={
                  (step === 1 && !canAdvanceStep1) ||
                  (step === 2 && !canAdvanceStep2)
                }
                onClick={() => setStep((s) => Math.min(3, s + 1))}
              >
                {t('study.next')}
                <ArrowRight className="h-4 w-4" />
              </Button>
            )}
          </div>
        </DialogContent>
      </Dialog>

      <DRSObjectPicker
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        onSelect={(obj) => setSelectedObject(obj)}
      />
    </>
  );
}
