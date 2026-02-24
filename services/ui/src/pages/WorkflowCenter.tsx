import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Play } from 'lucide-react';

export function WorkflowCenter() {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Workflow Center</h1>
          <p className="text-muted-foreground">Submit and monitor WES runs.</p>
        </div>
        <Button><Play className="mr-2 h-4 w-4" />Submit workflow</Button>
      </div>
      <Card>
        <CardHeader><CardTitle>Runs</CardTitle></CardHeader>
        <CardContent><p className="text-muted-foreground">Run list and detail.</p></CardContent>
      </Card>
    </div>
  );
}
