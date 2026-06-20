import { useEffect, useRef, useState, useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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
import { WORKFLOW_ENGINES, engineByWesType, guessEngineFromFilename } from '@/lib/workflowEngines';
import { useWdlDescriptor } from '@/hooks/useWdlDescriptor';
import { initParamValues, WorkflowParamForm } from '@/components/WorkflowParamForm';
import { parseWdlWorkflowInputs } from '@/lib/wdlInputs';
import { ErrorWithReport } from '@/components/ErrorWithReport';
import { useI18n } from '@/i18n/I18nProvider';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { buildFlatWorkflowParams, submitWorkflowRun } from '@/lib/wesSubmit';
import { isTrsDescriptorUrl, registerWorkflowInTrs } from '@/lib/trsRegister';
import { Play, Loader2, Upload } from 'lucide-react';

interface SubmitWorkflowDialogProps {
  disabled?: boolean;
  workspaceId?: string | null;
}

interface TrsTool {
  id: string;
  name?: string;
  versions?: Array<{ id: string; name?: string }>;
}

type SourceTab = 'curated' | 'trs' | 'custom' | 'upload';

export function SubmitWorkflowDialog({ disabled, workspaceId }: SubmitWorkflowDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const { data: adminConfig } = useAdminConfig();
  const fileRef = useRef<HTMLInputElement>(null);

  const serverAutoTrs = adminConfig?.compute?.wes_trs_auto_register !== false;

  const [open, setOpen] = useState(false);
  const [sourceTab, setSourceTab] = useState<SourceTab>('curated');
  const [curatedId, setCuratedId] = useState(CURATED_WORKFLOWS[0]?.id ?? '');
  const [trsToolId, setTrsToolId] = useState('');
  const [workflowUrl, setWorkflowUrl] = useState(CURATED_WORKFLOWS[0]?.workflowUrl ?? '');
  const [workflowType, setWorkflowType] = useState('WDL');
  const [engineId, setEngineId] = useState('wdl');
  const [uploadName, setUploadName] = useState('');
  const [workflowContent, setWorkflowContent] = useState('');
  const [uploadFileName, setUploadFileName] = useState('');
  const [registerInTrs, setRegisterInTrs] = useState(true);
  const [paramValues, setParamValues] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  const engine = WORKFLOW_ENGINES.find((e) => e.id === engineId) ?? WORKFLOW_ENGINES[0];

  const { data: trsTools } = useQuery({
    queryKey: ['trs', 'tools', 'submit'],
    queryFn: () => apiGet<TrsTool[]>('/ga4gh/trs/v2/tools'),
    enabled: open && sourceTab === 'trs',
    retry: false,
  });

  const tools = Array.isArray(trsTools) ? trsTools : [];
  const descriptorUrlForParams =
    sourceTab === 'upload' ? '' : workflowUrl;
  const { data: wdlParsedRemote, isLoading: wdlLoading } = useWdlDescriptor(
    descriptorUrlForParams,
    workflowType,
    open && sourceTab !== 'upload',
  );

  const wdlParsedUpload = useMemo(() => {
    if (sourceTab !== 'upload' || !workflowContent.trim()) return null;
    if (!workflowType.toLowerCase().includes('wdl')) return null;
    try {
      return parseWdlWorkflowInputs(workflowContent);
    } catch {
      return null;
    }
  }, [sourceTab, workflowContent, workflowType]);

  const wdlParsed = sourceTab === 'upload' ? wdlParsedUpload : wdlParsedRemote;
  const wdlLoadingEffective = sourceTab === 'upload' ? false : wdlLoading;

  useEffect(() => {
    if (!wdlParsed) {
      setParamValues({});
      return;
    }
    setParamValues(initParamValues(wdlParsed.inputs, false));
  }, [wdlParsed?.workflowName, wdlParsed?.inputs]);

  useEffect(() => {
    if (!open) return;
    setRegisterInTrs(serverAutoTrs);
  }, [open, serverAutoTrs]);

  const applyCurated = (id: string) => {
    const wf = CURATED_WORKFLOWS.find((c) => c.id === id);
    if (!wf) return;
    setCuratedId(id);
    setWorkflowUrl(wf.workflowUrl);
    setWorkflowType(wf.workflowType);
    setParamValues({});
    const eng = WORKFLOW_ENGINES.find((e) => e.wesType === wf.workflowType);
    if (eng) setEngineId(eng.id);
  };

  const applyTrsTool = (toolId: string) => {
    setTrsToolId(toolId);
    const tool = tools.find((x) => x.id === toolId);
    const version = tool?.versions?.[0];
    if (tool && version) {
      const desc = tool.id.includes('cwl') ? 'CWL' : tool.id.includes('snakemake') ? 'SMK' : tool.id.includes('nextflow') ? 'NFL' : 'WDL';
      setWorkflowUrl(trsDescriptorUrl(tool.id, version.id, desc));
      const eng = engineByWesType(desc === 'NFL' ? 'Nextflow' : desc === 'SMK' ? 'Snakemake' : desc);
      setWorkflowType(eng?.wesType ?? 'WDL');
      if (eng) setEngineId(eng.id);
    }
  };

  function onUploadFilePicked(file: File | undefined) {
    if (!file) return;
    setUploadFileName(file.name);
    const guessed = guessEngineFromFilename(file.name);
    if (guessed) {
      setEngineId(guessed.id);
      setWorkflowType(guessed.wesType);
    }
    if (!uploadName.trim()) {
      setUploadName(file.name.replace(/\.[^.]+$/, ''));
    }
    const reader = new FileReader();
    reader.onload = () => setWorkflowContent(String(reader.result ?? ''));
    reader.readAsText(file);
  }

  const submit = useMutation({
    mutationFn: async () => {
      let runUrl = workflowUrl.trim();
      let runType = workflowType;

      if (sourceTab === 'upload') {
        if (!workflowContent.trim()) {
          throw new Error(t('workflows.uploadEmpty'));
        }
        const registered = await registerWorkflowInTrs({
          name: uploadName.trim() || uploadFileName.replace(/\.[^.]+$/, '') || undefined,
          workflowContent: workflowContent.trim(),
          workflowType: engine.wesType,
        });
        runUrl = registered.descriptorUrl;
        runType = engine.wesType;
        void qc.invalidateQueries({ queryKey: ['trs', 'tools'] });
      } else if (
        sourceTab === 'custom' &&
        registerInTrs &&
        runUrl &&
        !isTrsDescriptorUrl(runUrl)
      ) {
        const registered = await registerWorkflowInTrs({
          name: uploadName.trim() || undefined,
          workflowUrl: runUrl,
          workflowType: runType,
        });
        runUrl = registered.descriptorUrl;
        void qc.invalidateQueries({ queryKey: ['trs', 'tools'] });
      }

      const workflowName =
        wdlParsed?.workflowName ??
        (CURATED_WORKFLOWS.find((c) => c.id === curatedId)?.paramPrefix ||
          uploadName.trim() ||
          'Workflow');
      const workflow_params =
        wdlParsed && wdlParsed.inputs.length > 0
          ? buildFlatWorkflowParams(workflowName, paramValues)
          : {};
      return submitWorkflowRun({
        workflowType: runType,
        workflowUrl: runUrl,
        workflowParams: workflow_params,
        workspaceId,
      });
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      setWorkflowContent('');
      setUploadFileName('');
      qc.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const showWdlForm = workflowType.toLowerCase().includes('wdl') && wdlParsed && wdlParsed.inputs.length > 0;
  const canRun =
    sourceTab === 'upload'
      ? workflowContent.trim().length > 0
      : workflowUrl.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button disabled={disabled} className="gap-2">
          <Play className="h-4 w-4" />
          {t('workflows.submit')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('workflows.dialogTitle')}</DialogTitle>
        </DialogHeader>
        <Tabs value={sourceTab} onValueChange={(v) => setSourceTab(v as SourceTab)}>
          <TabsList className="grid w-full grid-cols-2 gap-1 h-auto p-1">
            <TabsTrigger
              value="curated"
              className="text-xs px-2 py-2 h-auto whitespace-normal leading-snug text-center"
            >
              {t('workflows.sourceCurated')}
            </TabsTrigger>
            <TabsTrigger
              value="trs"
              className="text-xs px-2 py-2 h-auto whitespace-normal leading-snug text-center"
            >
              {t('workflows.sourceTrs')}
            </TabsTrigger>
            <TabsTrigger
              value="upload"
              className="text-xs px-2 py-2 h-auto whitespace-normal leading-snug text-center gap-1"
            >
              <Upload className="h-3 w-3 shrink-0" />
              {t('workflows.sourceUpload')}
            </TabsTrigger>
            <TabsTrigger
              value="custom"
              className="text-xs px-2 py-2 h-auto whitespace-normal leading-snug text-center"
            >
              {t('workflows.sourceCustom')}
            </TabsTrigger>
          </TabsList>
          <TabsContent value="curated" className="space-y-3 pt-2">
            <div className="space-y-2">
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
            </div>
          </TabsContent>
          <TabsContent value="trs" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label>{t('workflows.pickWorkflow')}</Label>
              <Select value={trsToolId} onValueChange={applyTrsTool}>
                <SelectTrigger>
                  <SelectValue placeholder={t('workflows.pickWorkflow')} />
                </SelectTrigger>
                <SelectContent>
                  {tools.map((tool) => (
                    <SelectItem key={tool.id} value={tool.id}>
                      {tool.name ?? tool.id}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </TabsContent>
          <TabsContent value="upload" className="space-y-3 pt-2">
            <p className="text-sm text-muted-foreground">{t('workflows.uploadHint')}</p>
            <div className="space-y-2">
              <Label>{t('workflows.engineLabel')}</Label>
              <Select
                value={engineId}
                onValueChange={(id) => {
                  setEngineId(id);
                  const eng = WORKFLOW_ENGINES.find((e) => e.id === id);
                  if (eng) setWorkflowType(eng.wesType);
                }}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {WORKFLOW_ENGINES.map((e) => (
                    <SelectItem key={e.id} value={e.id}>
                      {t(e.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="upload-wf-name">{t('workflows.uploadNameLabel')}</Label>
              <Input
                id="upload-wf-name"
                value={uploadName}
                onChange={(e) => setUploadName(e.target.value)}
                placeholder={t('workflows.uploadNamePlaceholder')}
              />
            </div>
            <input
              ref={fileRef}
              type="file"
              className="hidden"
              accept=".wdl,.cwl,.nf,.smk,.yaml,.yml,.groovy,.json,.txt"
              onChange={(e) => onUploadFilePicked(e.target.files?.[0])}
            />
            <Button type="button" variant="outline" className="w-full gap-2" onClick={() => fileRef.current?.click()}>
              <Upload className="h-4 w-4" />
              {t('tools.chooseFile')}
            </Button>
            {uploadFileName && (
              <p className="text-xs text-muted-foreground">{t('tools.fileSelected', { name: uploadFileName })}</p>
            )}
            <div className="space-y-2">
              <Label htmlFor="upload-content">{t('tools.pasteLabel')}</Label>
              <textarea
                id="upload-content"
                className="flex min-h-[100px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                value={workflowContent}
                onChange={(e) => setWorkflowContent(e.target.value)}
                placeholder={t('tools.pastePlaceholder')}
              />
            </div>
          </TabsContent>
          <TabsContent value="custom" className="space-y-3 pt-2">
            <div className="space-y-2">
              <Label>{t('workflows.engineLabel')}</Label>
              <Select value={workflowType} onValueChange={setWorkflowType}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {WORKFLOW_ENGINES.map((e) => (
                    <SelectItem key={e.id} value={e.wesType}>
                      {t(e.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="wf-url">{t('workflows.workflowUrlLabel')}</Label>
              <Input id="wf-url" value={workflowUrl} onChange={(e) => setWorkflowUrl(e.target.value)} />
            </div>
            <label className="flex items-start gap-2 text-sm cursor-pointer">
              <input
                type="checkbox"
                className="mt-1 rounded border-input"
                checked={registerInTrs}
                onChange={(e) => setRegisterInTrs(e.target.checked)}
              />
              <span>
                <span className="font-medium">{t('workflows.registerInTrs')}</span>
                <span className="block text-xs text-muted-foreground mt-0.5">
                  {serverAutoTrs ? t('workflows.registerInTrsHint') : t('workflows.registerInTrsServerOff')}
                </span>
              </span>
            </label>
          </TabsContent>
        </Tabs>

        {sourceTab === 'upload' && workflowContent.trim() && (
          <p className="text-xs text-muted-foreground">
            {t('workflows.registerInTrsHint')}
          </p>
        )}

        {wdlLoadingEffective && <p className="text-sm text-muted-foreground">{t('workflows.loadingParams')}</p>}
        {showWdlForm && (
          <WorkflowParamForm
            workflowName={wdlParsed.workflowName}
            inputs={wdlParsed.inputs}
            values={paramValues}
            onChange={setParamValues}
          />
        )}

        {error && (
          <ErrorWithReport
            errorMessage={error}
            context="wes-submit"
            lastApi={{ method: 'POST', path: '/ga4gh/wes/v1/runs' }}
          />
        )}
        <Button
          type="button"
          onClick={() => submit.mutate()}
          disabled={submit.isPending || !canRun}
          className="gap-2 w-full"
        >
          {submit.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
          {t('workflows.run')}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
