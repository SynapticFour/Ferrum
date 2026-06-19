import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { getPublishIndexStatus, publishDataset } from '@/api/publish';
import { useI18n } from '@/i18n/I18nProvider';
import { Share2 } from 'lucide-react';

interface PublishDatasetDialogProps {
  objectId: string;
  defaultName?: string;
  onPublished?: (adsDatasetId: string) => void;
}

async function pollVcfIndexUntilDone(objectId: string) {
  for (let i = 0; i < 30; i += 1) {
    const status = await getPublishIndexStatus(objectId);
    const s = status.vcf_index_status;
    if (!s || s === 'completed' || s === 'skipped' || s.startsWith('failed')) {
      return status;
    }
    await new Promise((r) => setTimeout(r, 2000));
  }
  return getPublishIndexStatus(objectId);
}

export function PublishDatasetDialog({
  objectId,
  defaultName,
  onPublished,
}: PublishDatasetDialogProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(defaultName ?? objectId);
  const [description, setDescription] = useState('');
  const [duoCodes, setDuoCodes] = useState('GRU');
  const [visibility, setVisibility] = useState<'institute' | 'public'>('institute');
  const [error, setError] = useState<string | null>(null);
  const [indexNote, setIndexNote] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () =>
      publishDataset({
        object_id: objectId,
        name: name.trim(),
        description: description.trim() || undefined,
        duo_codes: duoCodes
          .split(/[,\s]+/)
          .map((c) => c.trim())
          .filter(Boolean),
        visibility,
      }),
    onSuccess: async (res) => {
      setError(null);
      onPublished?.(res.ads_dataset_id);
      if (res.vcf_index_status === 'pending') {
        setIndexNote(t('data.publishVcfIndexing'));
        const final = await pollVcfIndexUntilDone(objectId);
        if (final.variants_indexed && final.variants_indexed > 0) {
          setIndexNote(
            t('data.publishVcfDone').replace('{n}', String(final.variants_indexed)),
          );
        } else {
          setIndexNote(null);
          setOpen(false);
        }
      } else {
        setOpen(false);
      }
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) setIndexNote(null);
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" className="gap-1">
          <Share2 className="h-3.5 w-3.5" />
          {t('data.publish')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('data.publishTitle')}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3 text-sm">
          <p className="text-muted-foreground">{t('data.publishHint')}</p>
          <div className="space-y-1">
            <Label htmlFor={`pub-name-${objectId}`}>{t('data.publishName')}</Label>
            <Input
              id={`pub-name-${objectId}`}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor={`pub-desc-${objectId}`}>{t('data.publishDescription')}</Label>
            <Input
              id={`pub-desc-${objectId}`}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor={`pub-duo-${objectId}`}>{t('data.publishDuo')}</Label>
            <Input
              id={`pub-duo-${objectId}`}
              value={duoCodes}
              onChange={(e) => setDuoCodes(e.target.value)}
              placeholder={t('data.publishDuoPlaceholder')}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor={`pub-vis-${objectId}`}>{t('data.publishVisibility')}</Label>
            <select
              id={`pub-vis-${objectId}`}
              className="w-full rounded-md border border-border bg-background px-3 py-2"
              value={visibility}
              onChange={(e) => setVisibility(e.target.value as 'institute' | 'public')}
            >
              <option value="institute">{t('data.publishVisibilityInstitute')}</option>
              <option value="public">{t('data.publishVisibilityPublic')}</option>
            </select>
          </div>
          {error && <p className="text-destructive text-sm">{error}</p>}
          {indexNote && <p className="text-muted-foreground text-sm">{indexNote}</p>}
        </div>
        <DialogFooter>
          <Button
            onClick={() => mutation.mutate()}
            disabled={mutation.isPending || !name.trim()}
          >
            {mutation.isPending || indexNote
              ? t('data.publishWorking')
              : t('data.publishSubmit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
