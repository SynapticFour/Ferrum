import { useCallback, useEffect, useRef, useState } from 'react';
import cytoscape, { type Core, type NodeSingular, type EventObject } from 'cytoscape';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { FileText, Cog, Copy, Download } from 'lucide-react';

export interface ProvenanceNodeData {
  id: string;
  label: string;
  type: 'drs_object' | 'wes_run';
}

export interface ProvenanceEdgeData {
  id: string;
  source: string;
  target: string;
  edge_type: string;
}

export interface ProvenanceGraphProps {
  nodes?: ProvenanceNodeData[];
  edges?: ProvenanceEdgeData[];
  mermaid?: string;
  cytoscapeJson?: { nodes: unknown[]; edges: unknown[] };
  height?: string;
  showExport?: boolean;
}

export function ProvenanceGraph({
  nodes = [],
  edges = [],
  mermaid = '',
  cytoscapeJson,
  height = '500px',
  showExport = true,
}: ProvenanceGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const [selectedNode, setSelectedNode] = useState<ProvenanceNodeData | null>(null);
  const [mermaidCopied, setMermaidCopied] = useState(false);

  const elements = cytoscapeJson ?? {
    nodes: nodes.map((n) => ({
      data: { id: n.id, label: n.label, type: n.type },
    })),
    edges: edges.map((e) => ({
      data: { id: e.id, source: e.source, target: e.target, edge_type: e.edge_type },
    })),
  };
  const hasElements = (elements.nodes as unknown[]).length > 0;

  const initCy = useCallback(() => {
    if (!containerRef.current || !hasElements) return;
    if (cyRef.current) cyRef.current.destroy();
    cyRef.current = cytoscape({
      container: containerRef.current,
      elements: elements as cytoscape.ElementsDefinition,
      style: [
        {
          selector: 'node',
          style: {
            label: 'data(label)',
            'text-valign': 'bottom',
            'text-margin-y': 4,
            'font-size': 10,
            width: 32,
            height: 32,
          },
        },
        {
          selector: 'node[type="drs_object"]',
          style: {
            'background-color': '#3b82f6',
            shape: 'round-rectangle',
          },
        },
        {
          selector: 'node[type="wes_run"]',
          style: {
            'background-color': '#f97316',
            shape: 'ellipse',
          },
        },
        {
          selector: 'edge',
          style: {
            label: 'data(edge_type)',
            'font-size': 8,
            'curve-style': 'bezier',
            'target-arrow-shape': 'triangle',
            width: 2,
          },
        },
      ],
      layout: { name: 'breadthfirst', directed: true, padding: 20 },
    });
    cyRef.current.on('tap', 'node', (ev: EventObject) => {
      const n = ev.target as NodeSingular;
      const data = n.data();
      setSelectedNode({
        id: data.id,
        label: data.label ?? data.id,
        type: (data.type as 'drs_object' | 'wes_run') ?? 'drs_object',
      });
    });
  }, [elements, hasElements]);

  useEffect(() => {
    initCy();
    return () => {
      if (cyRef.current) {
        cyRef.current.destroy();
        cyRef.current = null;
      }
    };
  }, [initCy]);

  const exportPng = () => {
    if (!cyRef.current) return;
    const blob = cyRef.current.png({ scale: 2, output: 'blob' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'provenance-graph.png';
    a.click();
    URL.revokeObjectURL(url);
  };

  const copyMermaid = () => {
    if (mermaid) {
      void navigator.clipboard.writeText(mermaid);
      setMermaidCopied(true);
      setTimeout(() => setMermaidCopied(false), 2000);
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        {showExport && (
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={exportPng}>
              <Download className="mr-1 h-4 w-4" />
              Export PNG
            </Button>
            {mermaid && (
              <Button variant="outline" size="sm" onClick={copyMermaid}>
                <Copy className="mr-1 h-4 w-4" />
                {mermaidCopied ? 'Copied!' : 'Export Mermaid'}
              </Button>
            )}
          </div>
        )}
      </div>
      <div className="flex gap-4">
        <div
          ref={containerRef}
          className="rounded-md border bg-muted/30"
          style={{ width: '100%', minHeight: height }}
        />
        {selectedNode && (
          <Card className="w-72 shrink-0">
            <CardHeader className="pb-2">
              <CardTitle className="flex items-center gap-2 text-sm">
                {selectedNode.type === 'drs_object' ? (
                  <FileText className="h-4 w-4" />
                ) : (
                  <Cog className="h-4 w-4" />
                )}
                {selectedNode.type === 'drs_object' ? 'DRS Object' : 'WES Run'}
              </CardTitle>
            </CardHeader>
            <CardContent className="text-sm">
              <p className="font-mono text-xs text-muted-foreground break-all">{selectedNode.id}</p>
              <p className="mt-1 font-medium">{selectedNode.label}</p>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
