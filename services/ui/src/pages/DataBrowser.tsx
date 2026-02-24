import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

export function DataBrowser() {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Data Browser</h1>
          <p className="text-muted-foreground">Browse and manage DRS objects.</p>
        </div>
        <Button>Upload</Button>
      </div>
      <Card>
        <CardHeader><CardTitle>Objects</CardTitle></CardHeader>
        <CardContent><p className="text-muted-foreground">Table view and filters.</p></CardContent>
      </Card>
    </div>
  );
}
