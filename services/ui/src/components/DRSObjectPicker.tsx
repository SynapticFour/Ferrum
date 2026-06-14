import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { apiGet } from '@/api/client';
import type { DrsObject } from '@/api/types';
import { useI18n } from '@/i18n/I18nProvider';

export function DRSObjectPicker({
  open,
  onClose,
  onSelect,
}: {
  open: boolean;
  onClose: () => void;
  onSelect: (obj: DrsObject) => void;
}) {
  const { t } = useI18n();
  const [query, setQuery] = useState('');

  const { data: objects, isLoading } = useQuery({
    queryKey: ['drs', 'objects', 'picker'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    enabled: open,
    retry: false,
  });

  const list = Array.isArray(objects) ? objects : [];

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (o) =>
        o.id.toLowerCase().includes(q) ||
        (o.name?.toLowerCase().includes(q) ?? false) ||
        (o.description?.toLowerCase().includes(q) ?? false),
    );
  }, [list, query]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>{t('picker.title')}</DialogTitle>
        </DialogHeader>
        <Input
          placeholder={t('picker.searchPlaceholder')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="min-h-0 flex-1 overflow-y-auto space-y-2 py-2">
          {isLoading && <p className="text-sm text-muted-foreground">{t('common.loading')}</p>}
          {!isLoading && list.length === 0 && (
            <p className="text-sm text-muted-foreground">{t('picker.empty')}</p>
          )}
          {!isLoading && list.length > 0 && filtered.length === 0 && (
            <p className="text-sm text-muted-foreground">{t('picker.noResults')}</p>
          )}
          {filtered.map((obj) => (
            <button
              key={obj.id}
              type="button"
              className="w-full rounded-lg border border-border p-3 text-left transition-colors hover:border-primary/40 hover:bg-muted/50"
              onClick={() => {
                onSelect(obj);
                onClose();
              }}
            >
              <p className="font-medium text-sm">{obj.name ?? obj.id}</p>
              <p className="text-xs text-muted-foreground font-mono truncate">{obj.id}</p>
              {obj.mime_type && (
                <p className="text-xs text-muted-foreground mt-1">{obj.mime_type}</p>
              )}
            </button>
          ))}
        </div>
        <Button variant="outline" onClick={onClose}>
          {t('common.cancel')}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
