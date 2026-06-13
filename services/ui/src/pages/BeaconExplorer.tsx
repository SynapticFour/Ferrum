import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPost } from '@/api/client';
import { Search, Loader2, AlertCircle } from 'lucide-react';

interface VariantQueryResponse {
  meta: Record<string, unknown>;
  response: { exists?: boolean; count?: number };
}

export function BeaconExplorer() {
  const [referenceName, setReferenceName] = useState('22');
  const [start, setStart] = useState('2000');
  const [end, setEnd] = useState('2000');
  const [referenceBases, setReferenceBases] = useState('T');
  const [alternateBases, setAlternateBases] = useState('G');
  const [assemblyId, setAssemblyId] = useState('GRCh37');
  const [federate, setFederate] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<VariantQueryResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleQuery() {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const path = federate
        ? `/ga4gh/beacon/v2/g_variants?federate=true&referenceName=${encodeURIComponent(referenceName)}&start=${start}&referenceBases=${referenceBases}&alternateBases=${alternateBases}`
        : '/ga4gh/beacon/v2/g_variants/query';
      const body = federate
        ? undefined
        : {
            meta: { apiVersion: 'v2.0.0' },
            query: {
              requestParameters: {
                assemblyId,
                referenceName,
                start: parseInt(start, 10),
                end: end ? parseInt(end, 10) : parseInt(start, 10),
                referenceBases: referenceBases || undefined,
                alternateBases: alternateBases || undefined,
                requestedGranularity: 'boolean',
              },
            },
          };
      const res = await apiPost<VariantQueryResponse>(path, body);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Beacon Explorer</h1>
        <p className="text-muted-foreground">Query genomic variants (GA4GH Beacon v2).</p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Variant query</CardTitle>
          <p className="text-sm text-muted-foreground">
            Exact SNV match (reference/alternate bases) or enable federation to fan out to configured peers.
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-3">
            <div className="space-y-2">
              <Label htmlFor="assembly">Assembly</Label>
              <Input id="assembly" value={assemblyId} onChange={(e) => setAssemblyId(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="ref">Reference name</Label>
              <Input id="ref" value={referenceName} onChange={(e) => setReferenceName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="start">Start position</Label>
              <Input id="start" type="number" value={start} onChange={(e) => setStart(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="refbases">Reference bases</Label>
              <Input id="refbases" value={referenceBases} onChange={(e) => setReferenceBases(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="altbases">Alternate bases</Label>
              <Input id="altbases" value={alternateBases} onChange={(e) => setAlternateBases(e.target.value)} />
            </div>
            <div className="flex items-end gap-2 pb-2">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={federate}
                  onChange={(e) => setFederate(e.target.checked)}
                />
                Federate to peers
              </label>
            </div>
          </div>
          <Button type="button" onClick={handleQuery} disabled={loading} className="gap-2">
            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
            Query
          </Button>
          {error && (
            <div className="flex items-center gap-2 rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              <AlertCircle className="h-4 w-4 shrink-0" />
              {error}
            </div>
          )}
          {result && (
            <div className="rounded-md border border-border bg-muted/30 p-4 text-sm">
              <p className="font-medium mb-2">Result</p>
              <pre className="overflow-auto text-xs">{JSON.stringify(result.response, null, 2)}</pre>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
