import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Loader2 } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';
import {
  submitAccessRequest,
  type DatasetCatalogEntry,
  type ResearchProject,
} from '@/api/access';

export function RequestAccessDialog({
  dataset,
  projects,
  researcherId,
  open,
  onOpenChange,
}: {
  dataset: DatasetCatalogEntry | null;
  projects: ResearchProject[];
  researcherId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [projectId, setProjectId] = useState('');
  const [justification, setJustification] = useState('');

  const mutation = useMutation({
    mutationFn: () =>
      submitAccessRequest(
        {
          researcher_id: researcherId,
          dataset_id: dataset!.id,
          project_id: projectId,
          justification: justification.trim() || undefined,
        },
        dataset!.ads_base_url,
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['access', 'requests'] });
      onOpenChange(false);
      setJustification('');
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('access.requestTitle')}</DialogTitle>
        </DialogHeader>
        {dataset && (
          <div className="space-y-4 text-sm">
            <p>
              <strong>{dataset.name}</strong>
              {dataset.dac_group && (
                <span className="text-muted-foreground"> · {dataset.dac_group}</span>
              )}
            </p>
            {dataset.description && (
              <p className="text-muted-foreground">{dataset.description}</p>
            )}
            <div className="flex flex-wrap gap-1">
              {dataset.duo_codes.map((c) => (
                <Badge key={c} variant="outline">
                  {c}
                </Badge>
              ))}
            </div>
            <div className="space-y-2">
              <Label htmlFor="access-project">{t('access.selectProject')}</Label>
              <select
                id="access-project"
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
              >
                <option value="">{t('access.chooseProject')}</option>
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} ({p.duo_codes.join(', ')})
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="access-justification">{t('access.justification')}</Label>
              <textarea
                id="access-justification"
                className="flex min-h-[80px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                value={justification}
                onChange={(e) => setJustification(e.target.value)}
                placeholder={t('access.justificationPlaceholder')}
                rows={3}
              />
            </div>
            {dataset.auto_approve_enabled && (
              <p className="text-xs text-muted-foreground">{t('access.autoApproveHint')}</p>
            )}
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('common.cancel')}
          </Button>
          <Button
            onClick={() => mutation.mutate()}
            disabled={!projectId || mutation.isPending}
          >
            {mutation.isPending ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              t('access.submitRequest')
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
