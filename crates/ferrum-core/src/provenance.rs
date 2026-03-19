//! Data provenance and lineage: DRS objects <-> WES runs as a queryable DAG.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Node type in the provenance graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    DrsObject,
    WesRun,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::DrsObject => "drs_object",
            NodeType::WesRun => "wes_run",
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "wes_run" => NodeType::WesRun,
            _ => NodeType::DrsObject,
        })
    }
}

/// Edge type: input (run consumed object), output (run produced object), derived_from (object derived from another).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Input,
    Output,
    DerivedFrom,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeType::Input => "input",
            EdgeType::Output => "output",
            EdgeType::DerivedFrom => "derived_from",
        }
    }
}

impl std::str::FromStr for EdgeType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "output" => EdgeType::Output,
            "derived_from" => EdgeType::DerivedFrom,
            _ => EdgeType::Input,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    pub id: String,
    pub from_type: NodeType,
    pub from_id: String,
    pub to_type: NodeType,
    pub to_id: String,
    pub edge_type: EdgeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProvenanceNode {
    DrsObject {
        id: String,
        name: Option<String>,
        size: i64,
        mime_type: Option<String>,
        created_at: Option<DateTime<Utc>>,
    },
    WesRun {
        id: String,
        workflow_type: Option<String>,
        workflow_url: Option<String>,
        state: Option<String>,
        created_at: Option<DateTime<Utc>>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvenanceGraph {
    pub nodes: Vec<ProvenanceNode>,
    pub edges: Vec<ProvenanceEdge>,
}

impl ProvenanceGraph {
    /// Serialize to Mermaid flowchart syntax.
    pub fn to_mermaid(&self) -> String {
        let mut lines = vec!["flowchart LR".to_string()];
        let node_id = |ty: &NodeType, id: &str| format!("{}_{}", ty.as_str(), id.replace('-', "_"));
        for n in &self.nodes {
            let (id, label) = match n {
                ProvenanceNode::DrsObject { id, name, .. } => (
                    node_id(&NodeType::DrsObject, id),
                    name.as_deref().unwrap_or(id).replace('"', "'"),
                ),
                ProvenanceNode::WesRun {
                    id,
                    workflow_type,
                    workflow_url,
                    ..
                } => {
                    let lbl = workflow_type
                        .as_deref()
                        .or(workflow_url.as_deref())
                        .unwrap_or(id);
                    (node_id(&NodeType::WesRun, id), lbl.replace('"', "'"))
                }
            };
            lines.push(format!("  {}[\"{}\"]", id, label));
        }
        for e in &self.edges {
            let from = node_id(&e.from_type, &e.from_id);
            let to = node_id(&e.to_type, &e.to_id);
            lines.push(format!("  {} -->|{}| {}", from, e.edge_type.as_str(), to));
        }
        lines.join("\n")
    }

    /// Serialize to Graphviz DOT format.
    pub fn to_dot(&self) -> String {
        let mut lines = vec![
            "digraph provenance {".to_string(),
            "  rankdir=LR;".to_string(),
        ];
        let node_id = |ty: &NodeType, id: &str| format!("{}_{}", ty.as_str(), id.replace('-', "_"));
        for n in &self.nodes {
            let (id, label, shape) = match n {
                ProvenanceNode::DrsObject { id, name, .. } => (
                    node_id(&NodeType::DrsObject, id),
                    name.as_deref().unwrap_or(id).replace('"', "\\\""),
                    "box",
                ),
                ProvenanceNode::WesRun {
                    id,
                    workflow_type,
                    workflow_url,
                    ..
                } => {
                    let lbl = workflow_type
                        .as_deref()
                        .or(workflow_url.as_deref())
                        .unwrap_or(id);
                    (
                        node_id(&NodeType::WesRun, id),
                        lbl.replace('"', "\\\""),
                        "ellipse",
                    )
                }
            };
            lines.push(format!("  {} [label=\"{}\", shape={}];", id, label, shape));
        }
        for e in &self.edges {
            let from = node_id(&e.from_type, &e.from_id);
            let to = node_id(&e.to_type, &e.to_id);
            lines.push(format!(
                "  {} -> {} [label=\"{}\"];",
                from,
                to,
                e.edge_type.as_str()
            ));
        }
        lines.push("}".to_string());
        lines.join("\n")
    }

    /// Cytoscape.js-compatible JSON for UI.
    pub fn to_cytoscape_json(&self) -> serde_json::Value {
        let node_id = |ty: &NodeType, id: &str| format!("{}_{}", ty.as_str(), id.replace('-', "_"));
        let nodes: Vec<serde_json::Value> = self
            .nodes
            .iter()
            .map(|n| {
                let (id, label, node_type) = match n {
                    ProvenanceNode::DrsObject {
                        id,
                        name,
                        size: _,
                        mime_type: _,
                        ..
                    } => (
                        node_id(&NodeType::DrsObject, id),
                        name.as_deref().unwrap_or(id).to_string(),
                        "drs_object",
                    ),
                    ProvenanceNode::WesRun {
                        id,
                        workflow_type,
                        workflow_url,
                        state: _,
                        ..
                    } => (
                        node_id(&NodeType::WesRun, id),
                        workflow_type
                            .as_deref()
                            .or(workflow_url.as_deref())
                            .unwrap_or(id)
                            .to_string(),
                        "wes_run",
                    ),
                };
                serde_json::json!({
                    "data": { "id": id, "label": label, "type": node_type }
                })
            })
            .collect();
        let edges: Vec<serde_json::Value> = self
            .edges
            .iter()
            .map(|e| {
                let from = node_id(&e.from_type, &e.from_id);
                let to = node_id(&e.to_type, &e.to_id);
                serde_json::json!({
                    "data": { "id": e.id, "source": from, "target": to, "edge_type": e.edge_type.as_str() }
                })
            })
            .collect();
        serde_json::json!({ "nodes": nodes, "edges": edges })
    }
}

/// Provenance store (PostgreSQL only; uses recursive lineage).
pub struct ProvenanceStore {
    pool: PgPool,
}

impl ProvenanceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn insert_edge(
        &self,
        from_type: NodeType,
        from_id: &str,
        to_type: NodeType,
        to_id: &str,
        edge_type: EdgeType,
        metadata: Option<serde_json::Value>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let pool = self.pool.clone();
        let id = ulid::Ulid::new().to_string();
        let from_id = from_id.to_string();
        let to_id = to_id.to_string();
        let from_type_str = from_type.as_str().to_string();
        let to_type_str = to_type.as_str().to_string();
        let edge_type_str = edge_type.as_str().to_string();
        let meta = metadata.unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        async move {
            sqlx::query(
                r#"INSERT INTO provenance_edges (id, from_type, from_id, to_type, to_id, edge_type, metadata)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(&id)
            .bind(&from_type_str)
            .bind(&from_id)
            .bind(&to_type_str)
            .bind(&to_id)
            .bind(&edge_type_str)
            .bind(&meta)
            .execute(&pool)
            .await?;
            Ok(())
        }
    }

    /// Record that a WES run consumed a DRS object as input.
    pub async fn record_wes_input(&self, run_id: &str, object_id: &str) -> Result<()> {
        self.insert_edge(
            NodeType::DrsObject,
            object_id,
            NodeType::WesRun,
            run_id,
            EdgeType::Input,
            None,
        )
        .await
    }

    /// Record that a WES run produced a DRS object as output.
    pub async fn record_wes_output(&self, run_id: &str, object_id: &str) -> Result<()> {
        self.insert_edge(
            NodeType::WesRun,
            run_id,
            NodeType::DrsObject,
            object_id,
            EdgeType::Output,
            None,
        )
        .await
    }

    /// Record that an object was derived from another (e.g. ingest with derived_from).
    pub async fn record_derived_from(
        &self,
        from_object_id: &str,
        to_object_id: &str,
    ) -> Result<()> {
        self.insert_edge(
            NodeType::DrsObject,
            from_object_id,
            NodeType::DrsObject,
            to_object_id,
            EdgeType::DerivedFrom,
            None,
        )
        .await
    }

    /// Fetch edges and build graph with node details from DB (drs_objects, wes_runs).
    async fn edges_to_graph(
        pool: &PgPool,
        edges: Vec<(String, String, String, String, String)>,
    ) -> Result<ProvenanceGraph> {
        use std::collections::HashSet;

        type DrsObjectRow = Option<(Option<String>, i64, Option<String>, Option<DateTime<Utc>>)>;
        type WesRunRow = Option<(
            Option<String>,
            Option<String>,
            Option<String>,
            Option<DateTime<Utc>>,
        )>;

        let mut node_ids = HashSet::new();
        for (ft, fid, tt, tid, _) in &edges {
            node_ids.insert((ft.as_str(), fid.as_str()));
            node_ids.insert((tt.as_str(), tid.as_str()));
        }
        let mut nodes = Vec::new();
        for (ty, id) in &node_ids {
            if *ty == "drs_object" {
                let row: DrsObjectRow = sqlx::query_as(
                    "SELECT name, size, mime_type, created_time FROM drs_objects WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(pool)
                .await?;
                if let Some((name, size, mime_type, created_at)) = row {
                    nodes.push(ProvenanceNode::DrsObject {
                        id: (*id).to_string(),
                        name,
                        size,
                        mime_type,
                        created_at,
                    });
                } else {
                    nodes.push(ProvenanceNode::DrsObject {
                        id: (*id).to_string(),
                        name: None,
                        size: 0,
                        mime_type: None,
                        created_at: None,
                    });
                }
            } else {
                let row: WesRunRow = sqlx::query_as(
                        "SELECT workflow_type, workflow_url, state, created_at FROM wes_runs WHERE run_id = $1",
                    )
                    .bind(id)
                    .fetch_optional(pool)
                    .await?;
                if let Some((workflow_type, workflow_url, state, created_at)) = row {
                    nodes.push(ProvenanceNode::WesRun {
                        id: (*id).to_string(),
                        workflow_type,
                        workflow_url,
                        state,
                        created_at,
                    });
                } else {
                    nodes.push(ProvenanceNode::WesRun {
                        id: (*id).to_string(),
                        workflow_type: None,
                        workflow_url: None,
                        state: None,
                        created_at: None,
                    });
                }
            }
        }
        let edge_list: Vec<ProvenanceEdge> = edges
            .into_iter()
            .map(
                |(from_type, from_id, to_type, to_id, edge_type)| ProvenanceEdge {
                    id: ulid::Ulid::new().to_string(),
                    from_type: from_type.parse().unwrap_or(NodeType::DrsObject),
                    from_id,
                    to_type: to_type.parse().unwrap_or(NodeType::DrsObject),
                    to_id,
                    edge_type: edge_type.parse().unwrap_or(EdgeType::Input),
                    created_at: None,
                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                },
            )
            .collect();
        Ok(ProvenanceGraph {
            nodes,
            edges: edge_list,
        })
    }

    /// Upstream lineage of a DRS object (what produced it, recursively).
    pub async fn upstream(&self, object_id: &str, max_depth: u32) -> Result<ProvenanceGraph> {
        let depth = max_depth.clamp(1, 20);
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            r#"SELECT from_type, from_id, to_type, to_id, edge_type
               FROM provenance_lineage
               WHERE to_type = 'drs_object' AND to_id = $1 AND depth <= $2
               ORDER BY depth"#,
        )
        .bind(object_id)
        .bind(depth as i32)
        .fetch_all(&self.pool)
        .await?;
        Self::edges_to_graph(&self.pool, rows).await
    }

    /// Downstream lineage (what was derived from this object).
    pub async fn downstream(&self, object_id: &str, max_depth: u32) -> Result<ProvenanceGraph> {
        let depth = max_depth.clamp(1, 20);
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            r#"SELECT from_type, from_id, to_type, to_id, edge_type
               FROM provenance_lineage
               WHERE from_type = 'drs_object' AND from_id = $1 AND depth <= $2
               ORDER BY depth"#,
        )
        .bind(object_id)
        .bind(depth as i32)
        .fetch_all(&self.pool)
        .await?;
        Self::edges_to_graph(&self.pool, rows).await
    }

    /// Both directions for an object.
    pub async fn both(&self, object_id: &str, max_depth: u32) -> Result<ProvenanceGraph> {
        let mut up = self.upstream(object_id, max_depth).await?;
        let down = self.downstream(object_id, max_depth).await?;
        up.nodes.extend(down.nodes);
        up.edges.extend(down.edges);
        Ok(up)
    }

    /// Complete lineage subgraph for a WES run: inputs, outputs, and their lineage.
    pub async fn run_lineage(&self, run_id: &str) -> Result<ProvenanceGraph> {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            r#"SELECT from_type, from_id, to_type, to_id, edge_type
               FROM provenance_edges
               WHERE (from_id = $1 AND from_type = 'wes_run')
                  OR (to_id = $1 AND to_type = 'wes_run')"#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Self::edges_to_graph(&self.pool, rows).await
    }

    /// Generic subgraph: root_id + root_type (drs_object | wes_run), direction, depth.
    pub async fn subgraph(
        &self,
        root_id: &str,
        root_type: &str,
        direction: &str,
        max_depth: u32,
    ) -> Result<ProvenanceGraph> {
        let depth = max_depth.clamp(1, 20);
        let rows = if root_type == "wes_run" {
            self.run_lineage(root_id).await?
        } else {
            match direction {
                "upstream" => self.upstream(root_id, depth).await?,
                "downstream" => self.downstream(root_id, depth).await?,
                _ => self.both(root_id, depth).await?,
            }
        };
        Ok(rows)
    }
}
