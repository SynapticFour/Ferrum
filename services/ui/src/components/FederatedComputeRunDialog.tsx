import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { Grant } from '@/api/access';
import { CURATED_WORKFLOWS } from '@/lib/workflows';
import { submitFederatedWorkflowRun } from '@/lib/wesSubmit';
import { useI18n } from '@/i18n/I18nProvider';
import { Loader2, Play } from 'lucide-react';

interface FederatedComputeRunDialogProps {
  grant: Grant;
}

export function FederatedComputeRunDialog({ grant }: FederatedComputeRunDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [curatedId, setCuratedId] = useState(CURATED_WORKFLOWS[0]?.id ?? '');
  const [error, setError] = useState<string | null>(null);

  const workflow = CURATED_WORKFLOWS.find((c) => c.id === curatedId) ?? CURATED_WORKFLOWS[0];

  const submit = useMutation({
    mutationFn: async () => {
      if (!workflow) throw new Error(t('access.computeRunNoWorkflow'));
      return submitFederatedWorkflowRun({
        workflowType: workflow.workflowType,
        workflowTypeVersion: workflow.workflowTypeVersion,
        workflowUrl: workflow.workflowUrl,
        workflowParams: {},
        computePoolId: grant.dataset_id,
        remoteWesBaseUrl: grant.remote_wes_base_url,
        federationOrigin: grant.federation_origin,
        adsBaseUrl: grant.ads_base_url,
      });
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  if (!workflow) return null;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline" size="sm" className="mt-2 gap-1">
          <Play className="h-3 w-3" />
          {t('access.computeRunSubmit')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t('access.computeRunTitle')}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">{t('access.computeRunHint')}</p>
        <div className="space-y-2">
          <Label>{t('workflows.workflowLabel')}</Label>
          <Select value={curatedId} onValueChange={setCuratedId}>
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
        {error && <p className="text-sm text-destructive">{error}</p>}
        <Button
          type="button"
          onClick={() => submit.mutate()}
          disabled={submit.isPending}
          className="w-full gap-2"
        >
          {submit.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
          {t('workflows.run')}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
