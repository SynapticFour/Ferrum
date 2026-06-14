import { useEffect, useRef, useState } from 'react';
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
  type RegisterToolPreset,
} from '@/lib/trsCatalogs';
import { Plus, Loader2, Upload, Link2 } from 'lucide-react';

type SourceMode = 'url' | 'file';

interface RegisterToolDialogProps {
  preset?: RegisterToolPreset | null;
  onPresetApplied?: () => void;
}

export function RegisterToolDialog({ preset, onPresetApplied }: RegisterToolDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
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

  useEffect(() => {
    if (!preset) return;
    setOpen(true);
    if (preset.name) setName(preset.name);
    if (preset.description) setDescription(preset.description);
    if (preset.workflowUrl) {
      setWorkflowUrl(preset.workflowUrl);
      setMode('url');
    }
    if (preset.workflowContent) {
      setWorkflowContent(preset.workflowContent);
      setMode('file');
    }
    if (preset.engineId) setEngineId(preset.engineId);
    if (preset.toolclass) setToolclass(preset.toolclass);
    onPresetApplied?.();
  }, [preset, onPresetApplied]);

  const register = useMutation({
    mutationFn: () => {
      const body: Record<string, string | undefined> = {
        name: name.trim() || undefined,
        description: description.trim() || undefined,
        organization: 'Ferrum UI',
        toolclass,
      };
      if (isWorkflow) {
        body.workflow_type = engine.wesType;
        body.workflow_type_version = '1.0';
      }
      if (mode === 'url') {
        body.workflow_url = workflowUrl.trim();
      } else {
        body.workflow_content = workflowContent.trim();
        if (workflowUrl.trim()) body.workflow_url = workflowUrl.trim();
      }
      return apiPost('/ga4gh/trs/v2/internal/register', body);
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['trs', 'tools'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const canSubmit =
    mode === 'url'
      ? workflowUrl.trim().length > 0
      : workflowContent.trim().length > 0;

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

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button className="gap-2">
          <Plus className="h-4 w-4" />
          {t('tools.registerTool')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('tools.registerTitle')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('tools.registerIntro')}</p>
        </DialogHeader>

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
              placeholder="https://…/workflow.wdl"
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
            <Button type="button" variant="outline" className="w-full gap-2" onClick={() => fileRef.current?.click()}>
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
          <Input id="tool-name" value={name} onChange={(e) => setName(e.target.value)} placeholder={t('tools.namePlaceholder')} />
        </div>
        <div className="space-y-2">
          <Label htmlFor="tool-desc">{t('tools.descLabel')}</Label>
          <Input id="tool-desc" value={description} onChange={(e) => setDescription(e.target.value)} placeholder={t('tools.descPlaceholder')} />
        </div>

        {error && <p className="text-sm text-destructive">{error}</p>}
        <Button
          type="button"
          className="w-full gap-2"
          disabled={!canSubmit || register.isPending}
          onClick={() => register.mutate()}
        >
          {register.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
          {t('tools.registerTool')}
        </Button>
        <p className="text-xs text-muted-foreground">{t('tools.afterRegister')}</p>
      </DialogContent>
    </Dialog>
  );
}
