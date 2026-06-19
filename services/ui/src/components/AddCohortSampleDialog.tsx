import { useState } from 'react';
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
import { apiGet, apiPost } from '@/api/client';
import { DRSObjectPicker } from '@/components/DRSObjectPicker';
import { useI18n } from '@/i18n/I18nProvider';
import type { DrsObject } from '@/api/types';
import { Plus, Loader2, FolderOpen } from 'lucide-react';

const COHORTS_BASE = '/cohorts/v1';

interface PhenotypeField {
  field_name: string;
  display_name: string;
  field_type: string;
  required: boolean;
}

export function AddCohortSampleDialog({
  cohortId,
  disabled,
}: {
  cohortId: string;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [sampleId, setSampleId] = useState('');
  const [sex, setSex] = useState('');
  const [sequencingType, setSequencingType] = useState('');
  const [picked, setPicked] = useState<DrsObject[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: schema } = useQuery({
    queryKey: ['cohort', 'phenotype-schema'],
    queryFn: () => apiGet<PhenotypeField[]>(`${COHORTS_BASE}/phenotype-schema`),
    enabled: open,
  });

  const add = useMutation({
    mutationFn: () => {
      const phenotype: Record<string, string> = {};
      if (sex.trim()) phenotype.sex = sex.trim();
      if (sequencingType.trim()) phenotype.sequencing_type = sequencingType.trim();
      return apiPost(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId)}/samples`, {
        samples: [
          {
            sample_id: sampleId.trim(),
            drs_object_ids: picked.map((o) => o.id),
            phenotype,
          },
        ],
      });
    },
    onSuccess: () => {
      setOpen(false);
      setSampleId('');
      setSex('');
      setSequencingType('');
      setPicked([]);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['cohort-samples', cohortId] });
      void qc.invalidateQueries({ queryKey: ['cohort-stats', cohortId] });
      void qc.invalidateQueries({ queryKey: ['cohort', cohortId] });
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger asChild>
          <Button disabled={disabled} className="gap-2">
            <Plus className="h-4 w-4" />
            {t('cohort.addSample')}
          </Button>
        </DialogTrigger>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{t('cohort.addSampleTitle')}</DialogTitle>
            <p className="text-sm text-muted-foreground">{t('cohort.addSampleHint')}</p>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>{t('cohort.sampleIdLabel')}</Label>
              <Input value={sampleId} onChange={(e) => setSampleId(e.target.value)} placeholder={t('cohort.sampleIdPlaceholder')} />
            </div>
            <div className="space-y-1">
              <Label>{t('cohort.linkedData')}</Label>
              <div className="flex gap-2">
                <Input readOnly value={picked.map((o) => o.name ?? o.id).join(', ')} placeholder={t('cohort.pickData')} />
                <Button type="button" variant="outline" onClick={() => setPickerOpen(true)}>
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label>{t('cohort.sexLabel')}</Label>
                <Input value={sex} onChange={(e) => setSex(e.target.value)} placeholder={t('cohort.sexPlaceholder')} />
              </div>
              <div className="space-y-1">
                <Label>{t('cohort.sequencingLabel')}</Label>
                <Input value={sequencingType} onChange={(e) => setSequencingType(e.target.value)} placeholder={t('cohort.sequencingPlaceholder')} />
              </div>
            </div>
            {schema && schema.length > 0 && (
              <p className="text-xs text-muted-foreground">
                {t('cohort.schemaHint', { count: String(schema.length) })}
              </p>
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button
              type="button"
              className="w-full gap-2"
              disabled={!sampleId.trim() || add.isPending}
              onClick={() => add.mutate()}
            >
              {add.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              {t('cohort.addSample')}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <DRSObjectPicker
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        onSelect={(obj) => {
          setPicked((prev) => (prev.some((x) => x.id === obj.id) ? prev : [...prev, obj]));
          setPickerOpen(false);
        }}
      />
    </>
  );
}
