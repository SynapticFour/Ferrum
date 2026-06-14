import { useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { useMutation, useQueryClient } from '@tanstack/react-query';
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { apiPostFormData } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import { Dna, Loader2, Upload } from 'lucide-react';

export function OntIngestDialog() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [format, setFormat] = useState('pod5');
  const [runId, setRunId] = useState('run-001');
  const [sampleId, setSampleId] = useState('sample-A');
  const [organism, setOrganism] = useState('Plasmodium_falciparum');
  const [fileName, setFileName] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [error, setError] = useState<string | null>(null);

  const ingest = useMutation({
    mutationFn: async () => {
      if (!file) throw new Error(t('ont.noFile'));
      const fd = new FormData();
      fd.append(
        'ont_metadata',
        JSON.stringify({
          format,
          source_path: file.name,
          run_id: runId.trim(),
          sample_id: sampleId.trim(),
          organism: organism.trim(),
          dorado_basecalled: false,
        }),
      );
      fd.append('file', file);
      return apiPostFormData<{ object_id?: string }>('/api/v1/ingest/ont', fd);
    },
    onSuccess: (res) => {
      setOpen(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['drs', 'objects'] });
      if (res?.object_id) {
        void (navigate as (opts: { to: string }) => void)({ to: `/data/objects/${res.object_id}` });
      }
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="gap-2">
          <Dna className="h-4 w-4" />
          {t('ont.ingest')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{t('ont.title')}</DialogTitle>
          <p className="text-sm text-muted-foreground">{t('ont.hint')}</p>
        </DialogHeader>
        <ol className="list-decimal list-inside text-sm text-muted-foreground space-y-1 rounded border bg-muted/20 p-3">
          <li>{t('ont.step1')}</li>
          <li>{t('ont.step2')}</li>
          <li>{t('ont.step3')}</li>
        </ol>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label>{t('ont.format')}</Label>
            <Select value={format} onValueChange={setFormat}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="pod5">POD5</SelectItem>
                <SelectItem value="fast5">FAST5</SelectItem>
                <SelectItem value="blow5">BLOW5</SelectItem>
                <SelectItem value="fastq">FASTQ</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <input
            ref={fileRef}
            type="file"
            className="hidden"
            accept=".pod5,.fast5,.blow5,.fastq,.fq,.gz"
            onChange={(e) => {
              const f = e.target.files?.[0];
              setFile(f ?? null);
              setFileName(f?.name ?? '');
            }}
          />
          <Button type="button" variant="outline" className="w-full gap-2" onClick={() => fileRef.current?.click()}>
            <Upload className="h-4 w-4" />
            {fileName || t('ont.chooseFile')}
          </Button>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label>{t('ont.runId')}</Label>
              <Input value={runId} onChange={(e) => setRunId(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>{t('ont.sampleId')}</Label>
              <Input value={sampleId} onChange={(e) => setSampleId(e.target.value)} />
            </div>
          </div>
          <div className="space-y-1">
            <Label>{t('ont.organism')}</Label>
            <Input value={organism} onChange={(e) => setOrganism(e.target.value)} />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <Button type="button" className="w-full gap-2" disabled={!file || ingest.isPending} onClick={() => ingest.mutate()}>
            {ingest.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
            {t('ont.submit')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
