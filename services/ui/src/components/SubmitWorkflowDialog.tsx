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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPost } from '@/api/client';
import { Play, Loader2 } from 'lucide-react';

interface SubmitWorkflowDialogProps {
  disabled?: boolean;
}

export function SubmitWorkflowDialog({ disabled }: SubmitWorkflowDialogProps) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [workflowUrl, setWorkflowUrl] = useState(
    'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl'
  );
  const [interval, setInterval] = useState('22:1700-2300');
  const [inputBam, setInputBam] = useState('');
  const [error, setError] = useState<string | null>(null);

  const submit = useMutation({
    mutationFn: async () => {
      const stream = (id: string) =>
        `${window.location.origin}/ga4gh/drs/v1/objects/${id}/stream`;
      const params: Record<string, string> = { interval };
      if (inputBam) {
        params.input_bam = stream(inputBam);
      }
      const body = {
        workflow_type: 'WDL',
        workflow_type_version: '1.0',
        workflow_url: workflowUrl,
        workflow_params: Object.fromEntries(
          Object.entries({
            'TinyGermlineHC.interval': interval,
            'TinyGermlineHC.input_bam': inputBam ? stream(inputBam) : undefined,
          }).filter(([, v]) => v)
        ),
        tags: { source: 'ferrum-ui' },
      };
      return apiPost<{ run_id?: string }>('/ga4gh/wes/v1/runs', body);
    },
    onSuccess: () => {
      setOpen(false);
      setError(null);
      qc.invalidateQueries({ queryKey: ['wes', 'runs'] });
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button disabled={disabled} className="gap-2">
          <Play className="h-4 w-4" />
          Submit workflow
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Submit WES run</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="wf-url">Workflow URL (WDL)</Label>
            <Input id="wf-url" value={workflowUrl} onChange={(e) => setWorkflowUrl(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="wf-interval">Interval</Label>
            <Input id="wf-interval" value={interval} onChange={(e) => setInterval(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="wf-bam">DRS object ID (input BAM, optional)</Label>
            <Input
              id="wf-bam"
              value={inputBam}
              onChange={(e) => setInputBam(e.target.value)}
              placeholder="Paste object ID from Data Browser"
            />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <Button
            type="button"
            onClick={() => submit.mutate()}
            disabled={submit.isPending || !workflowUrl}
            className="gap-2"
          >
            {submit.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
            Run
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
