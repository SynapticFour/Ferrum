import { lazy, Suspense } from 'react';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import type { ProvenanceGraphResponse } from '@/api/types';
import { useI18n } from '@/i18n/I18nProvider';

const ProvenanceGraph = lazy(() =>
  import('./ProvenanceGraph').then((m) => ({ default: m.ProvenanceGraph })),
);

function nodeId(type: string, id: string): string {
  return `${type}_${id.replace(/-/g, '_')}`;
}

export function ObjectLineageTab({ objectId }: { objectId: string }) {
  const { t } = useI18n();
  const { data, isLoading, error } = useQuery({
    queryKey: ['drs', 'provenance', objectId],
    queryFn: () =>
      apiGet<ProvenanceGraphResponse>(
        `/ga4gh/drs/v1/objects/${encodeURIComponent(objectId)}/provenance?direction=both&depth=10`
      ),
  });

  if (isLoading) return <p className="text-muted-foreground">{t('object.lineageLoading')}</p>;
  if (error) return <p className="text-destructive">{t('object.lineageFailed')}</p>;
  if (!data?.graph?.nodes?.length) return <p className="text-muted-foreground">{t('object.lineageEmpty')}</p>;

  const g = data.graph;
  const nodes = g.nodes.map((n) => ({
    id: nodeId(n.type, n.id),
    label: (n.name ?? n.workflow_type ?? n.workflow_url ?? n.id) as string,
    type: n.type as 'drs_object' | 'wes_run',
  }));
  const edges = g.edges.map((e, i) => ({
    id: e.id || `e${i}`,
    source: nodeId(e.from_type, e.from_id),
    target: nodeId(e.to_type, e.to_id),
    edge_type: e.edge_type,
  }));

  return (
    <Suspense fallback={<p className="text-muted-foreground">{t('object.lineageLoading')}</p>}>
      <ProvenanceGraph
        nodes={nodes}
        edges={edges}
        mermaid={g.mermaid}
        cytoscapeJson={g.cytoscape as { nodes: unknown[]; edges: unknown[] } | undefined}
        showExport
      />
    </Suspense>
  );
}
