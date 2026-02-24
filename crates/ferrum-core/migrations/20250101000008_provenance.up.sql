-- Provenance and lineage: DRS objects <-> WES runs (DAG)
CREATE TABLE IF NOT EXISTS provenance_edges (
    id          TEXT PRIMARY KEY,
    from_type   TEXT NOT NULL,
    from_id     TEXT NOT NULL,
    to_type     TEXT NOT NULL,
    to_id       TEXT NOT NULL,
    edge_type   TEXT NOT NULL,
    created_at  TIMESTAMPTZ DEFAULT now(),
    metadata    JSONB DEFAULT '{}'
);

CREATE INDEX idx_prov_from ON provenance_edges(from_type, from_id);
CREATE INDEX idx_prov_to   ON provenance_edges(to_type, to_id);

-- Recursive CTE view for full lineage traversal (PostgreSQL)
CREATE OR REPLACE VIEW provenance_lineage AS
WITH RECURSIVE lineage(from_type, from_id, to_type, to_id, edge_type, depth, path) AS (
    SELECT from_type, from_id, to_type, to_id, edge_type, 1,
           ARRAY[from_id || '::' || from_type || '->' || to_id || '::' || to_type]
    FROM provenance_edges
    UNION ALL
    SELECT e.from_type, e.from_id, l.to_type, l.to_id, e.edge_type,
           l.depth + 1, l.path || (e.from_id || '::' || e.from_type || '->' || e.to_id || '::' || e.to_type)
    FROM provenance_edges e
    JOIN lineage l ON e.to_id = l.from_id AND e.to_type = l.from_type
    WHERE l.depth < 20 AND NOT (e.from_id || '::' || e.from_type || '->' || e.to_id || '::' || e.to_type = ANY(l.path))
)
SELECT * FROM lineage;
