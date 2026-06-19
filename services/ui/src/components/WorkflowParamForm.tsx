import { useState } from 'react';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { DRSObjectPicker } from '@/components/DRSObjectPicker';
import type { DrsObject } from '@/api/types';
import { useI18n } from '@/i18n/I18nProvider';
import {
  defaultValueForInput,
  isPerSampleFileInput,
  type WdlInput,
} from '@/lib/wdlInputs';
import { drsStreamUrl } from '@/lib/wesSubmit';
import { FolderOpen } from 'lucide-react';

interface WorkflowParamFormProps {
  workflowName: string;
  inputs: WdlInput[];
  values: Record<string, string>;
  onChange: (values: Record<string, string>) => void;
  /** Hide per-sample File inputs (filled automatically in cohort batch runs). */
  hidePerSampleFiles?: boolean;
  /** Optional display labels for File inputs (e.g. DRS object name). */
  fileLabels?: Record<string, string>;
}

export function WorkflowParamForm({
  workflowName,
  inputs,
  values,
  onChange,
  hidePerSampleFiles = false,
  fileLabels = {},
}: WorkflowParamFormProps) {
  const { t } = useI18n();
  const [pickerFor, setPickerFor] = useState<string | null>(null);

  const visible = inputs.filter((inp) => {
    if (hidePerSampleFiles && inp.wdlType === 'File' && isPerSampleFileInput(inp.name)) return false;
    return true;
  });

  if (visible.length === 0) return null;

  function setValue(name: string, value: string) {
    onChange({ ...values, [name]: value });
  }

  function onPickFile(inputName: string, obj: DrsObject) {
    setValue(inputName, drsStreamUrl(obj.id));
    setPickerFor(null);
  }

  return (
    <div className="space-y-3 rounded-md border border-border/80 bg-muted/10 p-3">
      <p className="text-sm font-medium">
        {t('workflows.paramsTitle', { workflow: workflowName })}
      </p>
      {visible.map((inp) => {
        const id = `wf-param-${inp.name}`;
        const val = values[inp.name] ?? defaultValueForInput(inp);
        const display =
          fileLabels[inp.name] ||
          (val.startsWith('http') ? (val.match(/\/objects\/([^/]+)\/stream/)?.[1] ?? val.split('/').pop() ?? val) : val);
        if (inp.wdlType === 'File') {
          return (
            <div key={inp.name} className="space-y-1">
              <Label htmlFor={id}>
                {inp.name}
                {inp.optional ? ` (${t('workflows.optional')})` : ''}
              </Label>
              <div className="flex gap-2">
                <Input
                  id={id}
                  readOnly
                  value={display}
                  placeholder={t('workflows.pickData')}
                />
                <Button type="button" variant="outline" size="icon" onClick={() => setPickerFor(inp.name)}>
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>
          );
        }
        return (
          <div key={inp.name} className="space-y-1">
            <Label htmlFor={id}>
              {inp.name}
              <span className="text-muted-foreground font-normal ml-1">({inp.wdlType})</span>
            </Label>
            <Input
              id={id}
              value={val}
              onChange={(e) => setValue(inp.name, e.target.value)}
              type={inp.wdlType === 'Int' || inp.wdlType === 'Float' ? 'number' : 'text'}
            />
          </div>
        );
      })}
      {pickerFor && (
        <DRSObjectPicker
          open={!!pickerFor}
          onClose={() => setPickerFor(null)}
          onSelect={(obj) => onPickFile(pickerFor, obj)}
        />
      )}
    </div>
  );
}

export function initParamValues(inputs: WdlInput[], hidePerSampleFiles = false): Record<string, string> {
  const out: Record<string, string> = {};
  for (const inp of inputs) {
    if (hidePerSampleFiles && inp.wdlType === 'File' && isPerSampleFileInput(inp.name)) continue;
    out[inp.name] = defaultValueForInput(inp);
  }
  return out;
}
