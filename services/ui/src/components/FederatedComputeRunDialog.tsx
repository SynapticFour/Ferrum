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
import type { DatasetCatalogEntry, Grant } from '@/api/access';
import { CURATED_WORKFLOWS } from '@/lib/workflows';
import { submitFederatedWorkflowRun } from '@/lib/wesSubmit';
import { useI18n } from '@/i18n/I18nProvider';
import { Loader2, Play } from 'lucide-react';

export interface FederatedComputeTarget {
  dataset_id: string;
  dataset_name?: string;
  remote_wes_base_url?: string;
  federation_origin?: string;
  ads_base_url?: string;
}

function targetFromGrant(grant: Grant): FederatedComputeTarget {
  return {
    dataset_id: grant.dataset_id,
    dataset_name: grant.dataset_name,
    remote_wes_base_url: grant.remote_wes_base_url,
    federation_origin: grant.federation_origin,
    ads_base_url: grant.ads_base_url,
  };
}

function targetFromCatalog(entry: DatasetCatalogEntry): FederatedComputeTarget {
  return {
    dataset_id: entry.id,
    dataset_name: entry.name,
    remote_wes_base_url: entry.remote_wes_base_url,
    federation_origin: entry.federation_origin,
    ads_base_url: entry.ads_base_url,
  };
}

interface FederatedComputeRunDialogProps {
  grant?: Grant;
  catalogEntry?: DatasetCatalogEntry;
}

export function FederatedComputeRunDialog({ grant, catalogEntry }: FederatedComputeRunDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [curatedId, setCuratedId] = useState(CURATED_WORKFLOWS[0]?.id ?? '');
  const [error, setError] = useState<string | null>(null);

  const target = grant
    ? targetFromGrant(grant)
    : catalogEntry
      ? targetFromCatalog(catalogEntry)
      : null;

  const workflow = CURATED_WORKFLOWS.find((c) => c.id === curatedId) ?? CURATED_WORKFLOWS[0];

  const submit = useMutation({
    mutationFn: async () => {
      if (!target) throw new Error(t('access.computeRunNoWorkflow'));
      if (!workflow) throw new Error(t('access.computeRunNoWorkflow'));
      return submitFederatedWorkflowRun({
        workflowType: workflow.workflowType,
        workflowTypeVersion: workflow.workflowTypeVersion,
        workflowUrl: workflow.workflowUrl,
        workflowParams: {},
        computePoolId: target.dataset_id,
        remoteWesBaseUrl: target.remote_wes_base_url,
        federationOrigin: target.federation_origin,
        adsBaseUrl: target.ads_base_url,
      });
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  if (!target || !workflow) return null;
  if (!target.remote_wes_base_url && !target.federation_origin) return null;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline" size="sm" className="gap-1">
          <Play className="h-3 w-3" />
          {t('access.computeRunSubmit')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t('access.computeRunTitle')}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">{t('access.computeRunHint')}</p>
        {target.dataset_name && (
          <p className="text-sm font-medium">{target.dataset_name}</p>
        )}
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
