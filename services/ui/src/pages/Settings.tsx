import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { apiGet } from '@/api/client';

export function Settings() {
  const { data: config } = useQuery({
    queryKey: ['admin', 'config'],
    queryFn: () => apiGet<Record<string, unknown>>('/admin/config'),
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
