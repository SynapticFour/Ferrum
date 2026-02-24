import { Link, useParams } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import type { DrsObject } from '@/api/types';
import { ObjectLineageTab } from '@/components/ObjectLineageTab';
import { Button } from '@/components/ui/button';
import { ArrowLeft } from 'lucide-react';

export function ObjectDetailPage() {
  const { objectId } = useParams({ strict: false });
  const id = objectId ?? '';

  const { data: obj, isLoading, error } = useQuery({
    queryKey: ['drs', 'object', id],
    queryFn: () => apiGet<DrsObject>(`/ga4gh/drs/v1/objects/${encodeURIComponent(id)}`),
    enabled: !!id,
  });

  if (!id) return <p className="text-muted-foreground">No object ID.</p>;
  if (isLoading) return <p className="text-muted-foreground">Loading…</p>;
  if (error || !obj) return <p className="text-destructive">Object not found.</p>;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to="/data"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <h1 className="text-2xl font-bold">{obj.name ?? obj.id}</h1>
      </div>
      <Tabs defaultValue="details">
        <TabsList>
          <TabsTrigger value="details">Details</TabsTrigger>
          <TabsTrigger value="lineage">Lineage</TabsTrigger>
        </TabsList>
        <TabsContent value="details">
          <Card>
            <CardHeader><CardTitle>Object metadata</CardTitle></CardHeader>
            <CardContent className="text-sm space-y-1">
              <p><span className="text-muted-foreground">ID:</span> <code className="break-all">{obj.id}</code></p>
              {obj.size != null && <p><span className="text-muted-foreground">Size:</span> {obj.size}</p>}
              {obj.mime_type && <p><span className="text-muted-foreground">MIME type:</span> {obj.mime_type}</p>}
              {obj.description && <p><span className="text-muted-foreground">Description:</span> {obj.description}</p>}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="lineage">
          <ObjectLineageTab objectId={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
