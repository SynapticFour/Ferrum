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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { createProject } from '@/api/access';
import { useI18n } from '@/i18n/I18nProvider';
import { Loader2 } from 'lucide-react';

const COMMON_DUO_CODES = ['GRU', 'HMB', 'DS', 'NRES', 'PUB', 'GSO'];

export function CreateProjectDialog({
  researcherId,
  open,
  onOpenChange,
  onCreated,
}: {
  researcherId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated?: () => void;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [duoCodes, setDuoCodes] = useState<string[]>(['GRU']);

  const toggleDuo = (code: string) => {
    setDuoCodes((prev) =>
      prev.includes(code) ? prev.filter((c) => c !== code) : [...prev, code],
    );
  };

  const mutation = useMutation({
    mutationFn: () =>
      createProject({
        researcher_id: researcherId,
        name: name.trim(),
        description: description.trim() || undefined,
        duo_codes: duoCodes,
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['access', 'projects'] });
      onOpenChange(false);
      setName('');
      setDescription('');
      setDuoCodes(['GRU']);
      onCreated?.();
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('access.newProject')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('data.createProjectHint')}</p>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="proj-name">{t('access.projectName')}</Label>
            <Input id="proj-name" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="proj-desc">{t('access.projectDescription')}</Label>
            <textarea
              id="proj-desc"
              className="flex min-h-[60px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={2}
            />
          </div>
          <div className="space-y-2">
            <Label>{t('access.intendedUseDuo')}</Label>
            <div className="flex flex-wrap gap-2">
              {COMMON_DUO_CODES.map((code) => (
                <Button
                  key={code}
                  type="button"
                  size="sm"
                  variant={duoCodes.includes(code) ? 'default' : 'outline'}
                  onClick={() => toggleDuo(code)}
                >
                  {code}
                </Button>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('common.cancel')}
          </Button>
          <Button
            onClick={() => mutation.mutate()}
            disabled={!name.trim() || duoCodes.length === 0 || mutation.isPending}
          >
            {mutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : t('access.createProject')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
