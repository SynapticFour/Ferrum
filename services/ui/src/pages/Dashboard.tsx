import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ServiceHealthBadge } from '@/components/ServiceHealthBadge';

export function Dashboard() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">Overview of data, workflows, and system health.</p>
      </div>
      <div className="grid gap-4 md:grid-cols-4">
        <Card><CardHeader><CardTitle className="text-sm">DRS Objects</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Storage</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">—</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Active Runs</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">Tools</CardTitle></CardHeader><CardContent><div className="text-2xl font-bold">0</div></CardContent></Card>
      </div>
      <Card>
        <CardHeader><CardTitle>System health</CardTitle></CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          <ServiceHealthBadge status="up" label="Gateway" />
          <ServiceHealthBadge status="up" label="DRS" />
          <ServiceHealthBadge status="up" label="WES" />
        </CardContent>
      </Card>
    </div>
  );
}
