import { useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { apiGet, apiPost } from '@/api/client';
import type { DrsObject } from '@/api/types';
import { useI18n } from '@/i18n/I18nProvider';
import { parseSampleSheetText, resolveSheetDrsIds } from '@/lib/sampleSheet';
import { FileSpreadsheet, Loader2, Upload } from 'lucide-react';

const COHORTS_BASE = '/cohorts/v1';

export function SampleSheetImportDialog({
  cohortId,
  disabled,
}: {
  cohortId: string;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<ReturnType<typeof parseSampleSheetText>>([]);
  const [parseError, setParseError] = useState<string | null>(null);

  const { data: drsObjects } = useQuery({
    queryKey: ['drs', 'objects', 'sheet-import'],
    queryFn: () => apiGet<DrsObject[]>('/ga4gh/drs/v1/objects'),
    enabled: open,
  });

  const importMutation = useMutation({
    mutationFn: async () => {
      const list = Array.isArray(drsObjects) ? drsObjects : [];
      const byId = new Map(list.map((o) => [o.id, o]));
      const byName = new Map<string, string>();
      for (const o of list) {
        if (o.name) byName.set(o.name.toLowerCase(), o.id);
      }
      const resolved = resolveSheetDrsIds(preview, byId, byName);
      if (resolved.length === 0) throw new Error(t('cohort.sheetEmpty'));
      return apiPost(`${COHORTS_BASE}/cohorts/${encodeURIComponent(cohortId)}/samples`, {
        samples: resolved.map((r) => ({
          sample_id: r.sample_id,
          drs_object_ids: r.drs_object_ids,
          phenotype: r.phenotype,
        })),
      });
    },
    onSuccess: () => {
      setOpen(false);
      setPreview([]);
      setParseError(null);
      void qc.invalidateQueries({ queryKey: ['cohort-samples', cohortId] });
      void qc.invalidateQueries({ queryKey: ['cohort-stats', cohortId] });
      void qc.invalidateQueries({ queryKey: ['cohort', cohortId] });
    },
    onError: (e: Error) => setParseError(e.message),
  });

  function onFile(file: File | undefined) {
    if (!file) return;
    setParseError(null);
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const rows = parseSampleSheetText(String(reader.result ?? ''));
        setPreview(rows);
        if (!rows.length) setParseError(t('cohort.sheetEmpty'));
      } catch (e) {
        setPreview([]);
        setParseError(e instanceof Error ? e.message : t('cohort.sheetParseError'));
      }
    };
    reader.readAsText(file);
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="gap-2" disabled={disabled}>
          <FileSpreadsheet className="h-4 w-4" />
          {t('cohort.importSheet')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('cohort.importSheetTitle')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('cohort.importSheetHint')}</p>
        </DialogHeader>
        <input
          ref={fileRef}
          type="file"
          accept=".csv,.tsv,.txt"
          className="hidden"
          onChange={(e) => onFile(e.target.files?.[0])}
        />
        <Button type="button" variant="outline" className="w-full gap-2" onClick={() => fileRef.current?.click()}>
          <Upload className="h-4 w-4" />
          {t('cohort.chooseSheet')}
        </Button>
        <pre className="text-xs text-muted-foreground rounded border bg-muted/30 p-2 overflow-x-auto">
          {t('cohort.sheetExample')}
        </pre>
        {preview.length > 0 && (
          <div className="rounded border text-sm max-h-48 overflow-y-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b bg-muted/50">
                  <th className="p-2 text-left">sample_id</th>
                  <th className="p-2 text-left">DRS</th>
                  <th className="p-2 text-left">phenotype</th>
                </tr>
              </thead>
              <tbody>
                {preview.slice(0, 20).map((r) => (
                  <tr key={r.sample_id} className="border-b">
                    <td className="p-2 font-mono">{r.sample_id}</td>
                    <td className="p-2">{r.drs_object_ids.join(', ') || '—'}</td>
                    <td className="p-2 truncate max-w-[140px]">{JSON.stringify(r.phenotype)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {preview.length > 20 && (
              <p className="p-2 text-xs text-muted-foreground">+{preview.length - 20} more</p>
            )}
          </div>
        )}
        {parseError && <p className="text-sm text-destructive">{parseError}</p>}
        <Button
          type="button"
          className="w-full gap-2"
          disabled={!preview.length || importMutation.isPending}
          onClick={() => importMutation.mutate()}
        >
          {importMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <FileSpreadsheet className="h-4 w-4" />}
          {t('cohort.importSheetConfirm', { count: String(preview.length) })}
        </Button>
      </DialogContent>
    </Dialog>
  );
}
