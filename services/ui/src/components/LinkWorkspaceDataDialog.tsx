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
import { apiPost } from '@/api/client';
import { DRSObjectPicker } from '@/components/DRSObjectPicker';
import { useI18n } from '@/i18n/I18nProvider';
import type { DrsObject } from '@/api/types';
import { FolderInput, Loader2 } from 'lucide-react';

export interface LinkWorkspaceDataDialogProps {
  workspaceId: string;
  onSuccess?: (linked: number) => void;
  triggerVariant?: 'default' | 'outline';
}

export function LinkWorkspaceDataDialog({
  workspaceId,
  onSuccess,
  triggerVariant = 'outline',
}: LinkWorkspaceDataDialogProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [selected, setSelected] = useState<DrsObject[]>([]);
  const [error, setError] = useState<string | null>(null);

  const link = useMutation({
    mutationFn: () =>
      apiPost<{ linked: number }>(
        `/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}/objects/link`,
        { object_ids: selected.map((o) => o.id) },
      ),
    onSuccess: (data) => {
      void qc.invalidateQueries({ queryKey: ['drs', 'objects', 'workspace', workspaceId] });
      void qc.invalidateQueries({ queryKey: ['workspace', workspaceId, 'contents'] });
      setOpen(false);
      setSelected([]);
      setError(null);
      onSuccess?.(data.linked);
    },
    onError: (e: Error) => setError(e.message),
  });

  const toggleObject = (obj: DrsObject) => {
    setSelected((prev) => {
      const exists = prev.some((o) => o.id === obj.id);
      if (exists) return prev.filter((o) => o.id !== obj.id);
      return [...prev, obj];
    });
  };

  return (
    <>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger asChild>
          <Button variant={triggerVariant} className="gap-2">
            <FolderInput className="h-4 w-4" />
            {t('data.linkToWorkspace')}
          </Button>
        </DialogTrigger>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{t('data.linkTitle')}</DialogTitle>
            <p className="text-sm text-muted-foreground">{t('data.linkDescription')}</p>
          </DialogHeader>

          <div className="space-y-2">
            <p className="text-sm font-medium">{t('data.linkSelected', { count: String(selected.length) })}</p>
            {selected.length > 0 ? (
              <ul className="max-h-40 overflow-y-auto space-y-1 text-sm border rounded-md p-2">
                {selected.map((o) => (
                  <li key={o.id} className="flex justify-between gap-2">
                    <span className="truncate">{o.name ?? o.id}</span>
                    <button
                      type="button"
                      className="text-xs text-muted-foreground hover:text-destructive shrink-0"
                      onClick={() => setSelected((prev) => prev.filter((x) => x.id !== o.id))}
                    >
                      {t('common.remove')}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-sm text-muted-foreground">{t('data.linkNoneSelected')}</p>
            )}
            <Button type="button" variant="outline" onClick={() => setPickerOpen(true)}>
              {t('data.pickFromDrs')}
            </Button>
          </div>

          {error && <p className="text-sm text-destructive">{error}</p>}

          <Button
            type="button"
            disabled={selected.length === 0 || link.isPending}
            className="gap-2"
            onClick={() => link.mutate()}
          >
            {link.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <FolderInput className="h-4 w-4" />}
            {t('data.linkSubmit')}
          </Button>
        </DialogContent>
      </Dialog>

      <DRSObjectPicker
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        multiSelect
        selectedIds={selected.map((o) => o.id)}
        onSelect={toggleObject}
      />
    </>
  );
}
