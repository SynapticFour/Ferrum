import { useEffect, useRef, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useMutation, useQueryClient } from '@tanstack/react-query';
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
import { apiPost } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import {
  WORKFLOW_ENGINES,
  guessEngineFromFilename,
} from '@/lib/workflowEngines';
import {
  TRS_TOOL_CLASSES,
  TRS_IMPORT_PRESETS,
  type RegisterToolPreset,
} from '@/lib/trsCatalogs';
import {
  ArrowLeft,
  CheckCircle2,
  Globe,
  Loader2,
  Play,
  Plus,
  Sparkles,
  Upload,
  Link2,
  Download,
} from 'lucide-react';

type SourceMode = 'url' | 'file';
type Phase = 'choose' | 'form' | 'success';

interface RegisterToolDialogProps {
  preset?: RegisterToolPreset | null;
  onPresetApplied?: () => void;
}

function applyPresetToState(
  preset: RegisterToolPreset,
  setters: {
    setName: (v: string) => void;
    setDescription: (v: string) => void;
    setWorkflowUrl: (v: string) => void;
    setWorkflowContent: (v: string) => void;
    setMode: (v: SourceMode) => void;
    setEngineId: (v: string) => void;
    setToolclass: (v: string) => void;
  },
) {
  if (preset.name) setters.setName(preset.name);
  if (preset.description) setters.setDescription(preset.description);
  if (preset.workflowUrl) {
    setters.setWorkflowUrl(preset.workflowUrl);
    setters.setMode('url');
  }
  if (preset.workflowContent) {
    setters.setWorkflowContent(preset.workflowContent);
    setters.setMode('file');
  }
  if (preset.engineId) setters.setEngineId(preset.engineId);
  if (preset.toolclass) setters.setToolclass(preset.toolclass);
}

export function RegisterToolDialog({ preset, onPresetApplied }: RegisterToolDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>('choose');
  const [mode, setMode] = useState<SourceMode>('url');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [workflowUrl, setWorkflowUrl] = useState(
    'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl',
  );
  const [workflowContent, setWorkflowContent] = useState('');
  const [fileName, setFileName] = useState('');
  const [engineId, setEngineId] = useState('wdl');
  const [toolclass, setToolclass] = useState('Workflow');
  const [error, setError] = useState<string | null>(null);

  const engine = WORKFLOW_ENGINES.find((e) => e.id === engineId) ?? WORKFLOW_ENGINES[0];
  const isWorkflow = toolclass === 'Workflow';

  const setters = {
    setName,
    setDescription,
    setWorkflowUrl,
    setWorkflowContent,
    setMode,
    setEngineId,
    setToolclass,
  };

  useEffect(() => {
    if (!preset) return;
    setOpen(true);
    setPhase('form');
    applyPresetToState(preset, setters);
    onPresetApplied?.();
  }, [preset, onPresetApplied]);

  const resetOnClose = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setPhase('choose');
      setError(null);
    }
  };

  const register = useMutation({
    mutationFn: (override?: RegisterToolPreset) => {
      const resolvedName = override?.name ?? (name.trim() || undefined);
      const resolvedDesc = override?.description ?? (description.trim() || undefined);
      const resolvedUrl = override?.workflowUrl ?? workflowUrl.trim();
      const resolvedContent = override?.workflowContent ?? workflowContent.trim();
      const resolvedEngine = override?.engineId
        ? (WORKFLOW_ENGINES.find((e) => e.id === override.engineId) ?? engine)
        : engine;
      const resolvedClass = override?.toolclass ?? toolclass;
      const useUrl = override?.workflowUrl ? true : mode === 'url';

      const body: Record<string, string | undefined> = {
        name: resolvedName,
        description: resolvedDesc,
        organization: 'Ferrum',
        toolclass: resolvedClass,
      };
      if (resolvedClass === 'Workflow') {
        body.workflow_type = resolvedEngine.wesType;
        body.workflow_type_version = '1.0';
      }
      if (useUrl) {
        body.workflow_url = resolvedUrl;
      } else {
        body.workflow_content = resolvedContent;
        if (resolvedUrl) body.workflow_url = resolvedUrl;
      }
      return apiPost('/ga4gh/trs/v2/internal/register', body);
    },
    onSuccess: () => {
      setPhase('success');
      setError(null);
      void qc.invalidateQueries({ queryKey: ['trs', 'tools'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const canSubmit =
    mode === 'url' ? workflowUrl.trim().length > 0 : workflowContent.trim().length > 0;

  function onFilePicked(file: File | undefined) {
    if (!file) return;
    setFileName(file.name);
    const guessed = guessEngineFromFilename(file.name);
    if (guessed) setEngineId(guessed.id);
    if (!name.trim()) setName(file.name.replace(/\.[^.]+$/, ''));
    const reader = new FileReader();
    reader.onload = () => setWorkflowContent(String(reader.result ?? ''));
    reader.readAsText(file);
    setMode('file');
  }

  function startPresetImport(preset: (typeof TRS_IMPORT_PRESETS)[number], oneClick = false) {
    const resolved = {
      name: t(preset.nameKey),
      description: t(preset.sourceKey),
      workflowUrl: preset.workflowUrl,
      engineId: preset.engineId,
      toolclass: preset.toolclass,
    };
    if (oneClick) {
      register.mutate(resolved);
      return;
    }
    applyPresetToState(resolved, setters);
    setPhase('form');
    setError(null);
  }

  function startCustom() {
    setPhase('form');
    setError(null);
  }

  return (
    <Dialog open={open} onOpenChange={resetOnClose}>
      <DialogTrigger asChild>
        <Button className="gap-2" data-testid="register-tool-trigger">
          <Plus className="h-4 w-4" />
          {t('tools.registerTool')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-xl max-h-[90vh] overflow-y-auto">
        {phase === 'choose' && (
          <>
            <DialogHeader>
              <DialogTitle>{t('tools.registerTitle')}</DialogTitle>
              <p className="text-sm text-muted-foreground">{t('tools.choosePathIntro')}</p>
            </DialogHeader>

            <div className="grid gap-3">
              <div className="space-y-2">
                <p className="text-xs font-medium text-muted-foreground">{t('tools.importPreset')}</p>
                <div className="grid gap-3 sm:grid-cols-2">
                  {TRS_IMPORT_PRESETS.map((preset, index) => (
                    <button
                      key={preset.id}
                      type="button"
                      className={`rounded-lg border p-4 text-left transition-colors hover:bg-muted/30 ${
                        index === 0
                          ? 'border-primary/40 bg-primary/5 hover:bg-primary/10'
                          : 'border-border hover:border-primary/40'
                      }`}
                      onClick={() => startPresetImport(preset, index === 0)}
                      disabled={register.isPending}
                    >
                      <div className="flex items-start gap-3">
                        {index === 0 ? (
                          <Sparkles className="h-5 w-5 text-primary shrink-0 mt-0.5" />
                        ) : (
                          <Download className="h-5 w-5 shrink-0 mt-0.5 text-muted-foreground" />
                        )}
                        <div className="min-w-0">
                          <p className="font-medium">{t(preset.nameKey)}</p>
                          <p className="text-xs text-muted-foreground mt-1">{t(preset.sourceKey)}</p>
                          {index === 0 && (
                            <p className="text-xs text-primary mt-2">{t('tools.pathDemoHint')}</p>
                          )}
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              </div>

              <button
                type="button"
                className="rounded-lg border border-border p-4 text-left transition-colors hover:border-primary/40 hover:bg-muted/30"
                onClick={startCustom}
              >
                <div className="flex items-start gap-3">
                  <Upload className="h-5 w-5 shrink-0 mt-0.5" />
                  <div>
                    <p className="font-medium">{t('tools.pathCustomTitle')}</p>
                    <p className="text-sm text-muted-foreground mt-1">{t('tools.pathCustomHint')}</p>
                  </div>
                </div>
              </button>

              <button
                type="button"
                className="rounded-lg border border-border p-4 text-left transition-colors hover:border-primary/40 hover:bg-muted/30"
                onClick={() => {
                  setOpen(false);
                  window.setTimeout(() => {
                    document.getElementById('dockstore-panel')?.scrollIntoView({ behavior: 'smooth' });
                  }, 150);
                }}
              >
                <div className="flex items-start gap-3">
                  <Globe className="h-5 w-5 shrink-0 mt-0.5" />
                  <div>
                    <p className="font-medium">{t('tools.pathCatalogTitle')}</p>
                    <p className="text-sm text-muted-foreground mt-1">{t('tools.pathCatalogHint')}</p>
                  </div>
                </div>
              </button>
            </div>

            {register.isPending && (
              <p className="text-sm text-muted-foreground flex items-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                {t('tools.registering')}
              </p>
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
          </>
        )}

        {phase === 'form' && (
          <>
            <DialogHeader>
              <DialogTitle>{t('tools.registerTitle')}</DialogTitle>
              <p className="text-sm text-muted-foreground">{t('tools.registerIntro')}</p>
            </DialogHeader>

            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="w-fit gap-1 -mt-2"
              onClick={() => setPhase('choose')}
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              {t('tools.backToChoose')}
            </Button>

            <ol className="list-decimal list-inside space-y-1 text-sm text-muted-foreground rounded-md border border-border/80 bg-muted/20 p-3">
              <li>{t('tools.stepRegister')}</li>
              <li>{t('tools.stepCatalog')}</li>
              <li>{t('tools.stepRun')}</li>
            </ol>

            <div className="space-y-2">
              <Label>{t('tools.toolclassLabel')}</Label>
              <Select value={toolclass} onValueChange={setToolclass}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TRS_TOOL_CLASSES.map((c) => (
                    <SelectItem key={c.id} value={c.id}>
                      {t(c.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground">
                {isWorkflow ? t('tools.toolclassWorkflowHint') : t('tools.toolclassOtherHint')}
              </p>
            </div>

            {isWorkflow && (
              <div className="space-y-2">
                <Label>{t('tools.engineLabel')}</Label>
                <Select value={engineId} onValueChange={setEngineId}>
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
                <p className="text-xs text-muted-foreground">{t(engine.hintKey)}</p>
              </div>
            )}

            <Tabs value={mode} onValueChange={(v) => setMode(v as SourceMode)}>
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="url" className="gap-1">
                  <Link2 className="h-3.5 w-3.5" />
                  {t('tools.byUrl')}
                </TabsTrigger>
                <TabsTrigger value="file" className="gap-1">
                  <Upload className="h-3.5 w-3.5" />
                  {t('tools.byFile')}
                </TabsTrigger>
              </TabsList>
              <TabsContent value="url" className="space-y-2 pt-2">
                <Label htmlFor="tool-url">{t('tools.descriptorUrlLabel')}</Label>
                <Input
                  id="tool-url"
                  value={workflowUrl}
                  onChange={(e) => setWorkflowUrl(e.target.value)}
                  placeholder={t('tools.descriptorUrlPlaceholder')}
                />
                <p className="text-xs text-muted-foreground">{t('tools.urlHint')}</p>
              </TabsContent>
              <TabsContent value="file" className="space-y-2 pt-2">
                <input
                  ref={fileRef}
                  type="file"
                  className="hidden"
                  accept=".wdl,.cwl,.nf,.smk,.yaml,.yml,.groovy,.json,.txt"
                  onChange={(e) => onFilePicked(e.target.files?.[0])}
                />
                <Button
                  type="button"
                  variant="outline"
                  className="w-full gap-2"
                  onClick={() => fileRef.current?.click()}
                >
                  <Upload className="h-4 w-4" />
                  {t('tools.chooseFile')}
                </Button>
                {fileName && (
                  <p className="text-xs text-muted-foreground">
                    {t('tools.fileSelected', { name: fileName })}
                  </p>
                )}
                <Label htmlFor="tool-content">{t('tools.pasteLabel')}</Label>
                <textarea
                  id="tool-content"
                  className="flex min-h-[120px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                  value={workflowContent}
                  onChange={(e) => setWorkflowContent(e.target.value)}
                  placeholder={t('tools.pastePlaceholder')}
                />
              </TabsContent>
            </Tabs>

            <div className="space-y-2">
              <Label htmlFor="tool-name">{t('tools.nameLabel')}</Label>
              <Input
                id="tool-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('tools.namePlaceholder')}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="tool-desc">{t('tools.descLabel')}</Label>
              <Input
                id="tool-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('tools.descPlaceholder')}
              />
            </div>

            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button
              type="button"
              className="w-full gap-2"
              disabled={!canSubmit || register.isPending}
              onClick={() => register.mutate(undefined)}
            >
              {register.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
              {t('tools.registerTool')}
            </Button>
          </>
        )}

        {phase === 'success' && (
          <div className="space-y-4 py-2">
            <div className="flex items-start gap-3 rounded-md border border-emerald-500/40 bg-emerald-500/10 p-4">
              <CheckCircle2 className="h-6 w-6 text-emerald-500 shrink-0" />
              <div>
                <p className="font-medium">{t('tools.registerSuccessTitle')}</p>
                <p className="text-sm text-muted-foreground mt-1">{t('tools.registerSuccessBody')}</p>
              </div>
            </div>
            <Button asChild className="w-full gap-2">
              <Link to={'/workflows' as any} onClick={() => resetOnClose(false)}>
                <Play className="h-4 w-4" />
                {t('tools.goRunAnalysisNow')}
              </Link>
            </Button>
            <Button variant="outline" className="w-full" onClick={() => resetOnClose(false)}>
              {t('common.dismiss')}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
