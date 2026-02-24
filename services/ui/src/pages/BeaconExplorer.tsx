import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

export function BeaconExplorer() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Beacon Explorer</h1>
        <p className="text-muted-foreground">Query genomic variants.</p>
      </div>
      <Card>
        <CardHeader><CardTitle>Variant query</CardTitle></CardHeader>
        <CardContent><Button>Query</Button></CardContent>
      </Card>
    </div>
  );
}
