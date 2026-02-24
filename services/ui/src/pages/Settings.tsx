import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiGet } from '@/api/client';

interface CacheStats {
  total_entries: number;
  total_size_bytes: number;
  hit_rate_7d: number;
  top_cached_tasks: Array<{ task_name: string; hits: number; size_gb: number }>;
}

export function Settings() {
  const { data: config } = useQuery({
    queryKey: ['admin', 'config'],
    queryFn: () => apiGet<Record<string, unknown>>('/admin/config'),
    retry: false,
  });
  const { data: cacheStats } = useQuery({
    queryKey: ['wes', 'cache', 'stats'],
    queryFn: () => apiGet<CacheStats>('/ga4gh/wes/v1/cache/stats'),
    retry: false,
  });

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">Service configuration and profile.</p>
      </div>

      <Tabs defaultValue="config">
        <TabsList>
          <TabsTrigger value="config">Configuration</TabsTrigger>
          <TabsTrigger value="storage">Storage</TabsTrigger>
          <TabsTrigger value="keys">Encryption keys</TabsTrigger>
          <TabsTrigger value="cache">Workflow cache</TabsTrigger>
          <TabsTrigger value="profile">Profile</TabsTrigger>
        </TabsList>
        <TabsContent value="config">
          <Card>
            <CardHeader>
              <CardTitle>Service configuration</CardTitle>
            </CardHeader>
            <CardContent>
              <pre className="overflow-auto rounded-md bg-muted p-4 text-xs">{config ? JSON.stringify(config, null, 2) : 'Not available.'}</pre>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="storage">
          <Card>
            <CardHeader>
              <CardTitle>Storage backend</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground">Storage backend status.</p>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="keys">
          <Card>
            <CardHeader>
              <CardTitle>Encryption keys</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground">View key IDs, generate new keys. Private keys are never shown.</p>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="cache">
          <Card>
            <CardHeader>
              <CardTitle>Workflow cache</CardTitle>
            </CardHeader>
            <CardContent>
              {cacheStats != null ? (
                <div className="space-y-2 text-sm">
                  <p><span className="font-medium">Total entries:</span> {cacheStats.total_entries}</p>
                  <p><span className="font-medium">Total size:</span> {(cacheStats.total_size_bytes / 1e9).toFixed(2)} GB</p>
                  <p><span className="font-medium">7-day hit rate:</span> {(cacheStats.hit_rate_7d * 100).toFixed(1)}%</p>
                  {cacheStats.top_cached_tasks?.length > 0 && (
                    <div className="mt-4">
                      <p className="font-medium mb-2">Top cached tasks</p>
                      <ul className="list-disc pl-4">
                        {cacheStats.top_cached_tasks.slice(0, 10).map((t, i) => (
                          <li key={i}>{t.task_name}: {t.hits} hits, {t.size_gb.toFixed(1)} GB</li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              ) : (
                <p className="text-muted-foreground">Cache stats not available (WES cache may be disabled).</p>
              )}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="profile">
          <Card>
            <CardHeader>
              <CardTitle>Profile</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground">User profile with own passport/visa viewer.</p>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
