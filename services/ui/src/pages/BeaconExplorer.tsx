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
  const [referenceName, setReferenceName] = useState('1');
  const [start, setStart] = useState('10000');
  const [end, setEnd] = useState('20000');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<VariantQueryResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleQuery() {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const res = await apiPost<VariantQueryResponse>('/ga4gh/beacon/v2/g_variants/query', {
        reference_name: referenceName || undefined,
        start: start ? parseInt(start, 10) : undefined,
        end: end ? parseInt(end, 10) : undefined,
      });
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
            Enter reference name (e.g. 1, chr1) and position range. The Beacon returns whether variants exist in that region.
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-3">
            <div className="space-y-2">
              <Label htmlFor="ref">Reference name</Label>
              <Input
                id="ref"
                value={referenceName}
                onChange={(e) => setReferenceName(e.target.value)}
                placeholder="e.g. 1"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="start">Start position</Label>
              <Input
                id="start"
                type="number"
                value={start}
                onChange={(e) => setStart(e.target.value)}
                placeholder="e.g. 10000"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="end">End position</Label>
              <Input
                id="end"
                type="number"
                value={end}
                onChange={(e) => setEnd(e.target.value)}
                placeholder="e.g. 20000"
              />
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
              <pre className="overflow-auto text-xs">
                {JSON.stringify(result.response, null, 2)}
              </pre>
              {result.response.exists != null && (
                <p className="mt-2">
                  Variants in this region: <strong>{result.response.exists ? 'Yes' : 'No'}</strong>
                  {result.response.count != null && ` (count: ${result.response.count})`}
                </p>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
