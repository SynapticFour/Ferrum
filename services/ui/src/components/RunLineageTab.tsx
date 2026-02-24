import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import type { ProvenanceGraphResponse } from '@/api/types';
import { ProvenanceGraph } from './ProvenanceGraph';

function nodeId(type: string, id: string): string {
  return `${type}_${id.replace(/-/g, '_')}`;
}

export function RunLineageTab({ runId }: { runId: string }) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['wes', 'provenance', runId],
    queryFn: () => apiGet<ProvenanceGraphResponse>(`/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/provenance`),
  });

  if (isLoading) return <p className="text-muted-foreground">Loading lineage…</p>;
  if (error) return <p className="text-destructive">Failed to load lineage.</p>;
  if (!data?.graph) return <p className="text-muted-foreground">No provenance data.</p>;

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
    <ProvenanceGraph
      nodes={nodes}
      edges={edges}
      mermaid={g.mermaid}
      cytoscapeJson={g.cytoscape as { nodes: unknown[]; edges: unknown[] } | undefined}
      showExport
    />
  );
}
