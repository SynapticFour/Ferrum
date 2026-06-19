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
import { Check } from 'lucide-react';

export function DRSObjectPicker({
  open,
  onClose,
  onSelect,
  multiSelect,
  selectedIds = [],
}: {
  open: boolean;
  onClose: () => void;
  onSelect: (obj: DrsObject) => void;
  multiSelect?: boolean;
  selectedIds?: string[];
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
  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);

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
          {multiSelect && (
            <p className="text-sm text-muted-foreground">{t('picker.multiHint')}</p>
          )}
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
          {filtered.map((obj) => {
            const isSelected = selectedSet.has(obj.id);
            return (
              <button
                key={obj.id}
                type="button"
                className={`w-full rounded-lg border p-3 text-left transition-colors hover:border-primary/40 hover:bg-muted/50 ${
                  isSelected ? 'border-primary bg-primary/5' : 'border-border'
                }`}
                onClick={() => {
                  onSelect(obj);
                  if (!multiSelect) onClose();
                }}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="font-medium text-sm">{obj.name ?? obj.id}</p>
                    <p className="text-xs text-muted-foreground font-mono truncate">{obj.id}</p>
                    {obj.mime_type && (
                      <p className="text-xs text-muted-foreground mt-1">{obj.mime_type}</p>
                    )}
                  </div>
                  {multiSelect && isSelected && (
                    <Check className="h-4 w-4 text-primary shrink-0" />
                  )}
                </div>
              </button>
            );
          })}
        </div>
        <Button variant="outline" onClick={onClose}>
          {multiSelect ? t('common.done') : t('common.cancel')}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
