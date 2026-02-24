//! Run metrics collection and cost estimation (WES/TES).
//! Purely based on wall-clock × configured resource price; no cloud billing API.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

/// Pricing configuration snapshot for display and reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingSnapshot {
    pub enabled: bool,
    pub currency: String,
    pub cpu_core_hour: f64,
    pub memory_gb_hour: f64,
    pub storage_gb_month: f64,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub tiers: HashMap<String, TierRates>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRates {
    pub cpu_core_hour: Option<f64>,
    pub memory_gb_hour: Option<f64>,
}

impl PricingSnapshot {
    pub fn from_config(cfg: &ferrum_core::PricingConfig) -> Self {
        let tiers = cfg
            .tiers
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    TierRates {
                        cpu_core_hour: v.cpu_core_hour,
                        memory_gb_hour: v.memory_gb_hour,
                    },
                )
            })
            .collect();
        Self {
            enabled: cfg.enabled,
            currency: cfg.currency.clone(),
            cpu_core_hour: cfg.cpu_core_hour,
            memory_gb_hour: cfg.memory_gb_hour,
            storage_gb_month: cfg.storage_gb_month,
            tiers,
        }
    }

    /// Cost in currency units for given cpu_seconds and memory_gb_h.
    pub fn cost(&self, cpu_seconds: f64, memory_gb_h: f64, _tier: Option<&str>) -> f64 {
        if !self.enabled {
            return 0.0;
        }
        let cpu_h = cpu_seconds / 3600.0;
        let cpu_rate = _tier
            .and_then(|t| self.tiers.get(t))
            .and_then(|r| r.cpu_core_hour)
            .unwrap_or(self.cpu_core_hour);
        let mem_rate = _tier
            .and_then(|t| self.tiers.get(t))
            .and_then(|r| r.memory_gb_hour)
            .unwrap_or(self.memory_gb_hour);
        cpu_h * cpu_rate + memory_gb_h * mem_rate
    }
}

/// One sample of task resource usage (e.g. every 30s).
#[derive(Debug, Clone)]
pub struct TaskSample {
    pub task_id: String,
    pub task_name: String,
    pub cpu_pct: f64,
    pub memory_mb: i64,
    pub read_bytes: i64,
    pub write_bytes: i64,
    pub timestamp: DateTime<Utc>,
}

/// Per-task cost breakdown in run summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCostBreakdown {
    pub task_id: String,
    pub task_name: String,
    pub wall_seconds: i64,
    pub cpu_seconds: f64,
    pub memory_gb_h: f64,
    pub peak_memory_mb: i64,
    pub estimated_cost: f64,
    pub exit_code: Option<i32>,
}

/// Full run cost summary with per-task breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCostSummary {
    pub run_id: String,
    pub total_wall_seconds: i64,
    pub total_cpu_seconds: f64,
    pub total_memory_gb_h: f64,
    pub peak_memory_mb: i64,
    pub total_read_gb: f64,
    pub total_write_gb: f64,
    pub estimated_cost_usd: f64,
    pub breakdown: Vec<TaskCostBreakdown>,
}

/// Pre-submit cost estimate from workflow_engine_params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub currency: String,
    /// Estimated amount (min–max or single value).
    pub low_usd: f64,
    pub high_usd: Option<f64>,
    pub assumptions: String,
}

pub struct MetricsCollector {
    pool: PgPool,
    pricing: ferrum_core::PricingConfig,
}

impl MetricsCollector {
    pub fn new(pool: PgPool, pricing: ferrum_core::PricingConfig) -> Self {
        Self { pool, pricing }
    }

    /// Called periodically (e.g. every 30s) during task execution. Upserts task_metrics by (run_id, task_id) and appends sample.
    pub async fn record_task_sample(
        &self,
        run_id: &str,
        sample: TaskSample,
        cpu_requested: f64,
        memory_requested_mb: i64,
        executor: &str,
        node_hostname: Option<&str>,
    ) -> Result<()> {
        let id = ulid::Ulid::new().to_string();
        let sample_json = serde_json::json!({
            "ts": sample.timestamp.to_rfc3339(),
            "cpu_pct": sample.cpu_pct,
            "memory_mb": sample.memory_mb,
            "read_bytes": sample.read_bytes,
            "write_bytes": sample.write_bytes
        });

        sqlx::query(
            r#"INSERT INTO task_metrics (id, run_id, task_id, task_name, started_at, cpu_requested, cpu_peak_pct, memory_requested_mb, memory_peak_mb, read_bytes, write_bytes, executor, node_hostname, samples)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::jsonb)
               ON CONFLICT (run_id, task_id) DO UPDATE SET
                 task_name = EXCLUDED.task_name,
                 cpu_peak_pct = GREATEST(COALESCE(task_metrics.cpu_peak_pct, 0), EXCLUDED.cpu_peak_pct),
                 memory_peak_mb = GREATEST(COALESCE(task_metrics.memory_peak_mb, 0), EXCLUDED.memory_peak_mb),
                 read_bytes = COALESCE(task_metrics.read_bytes, 0) + EXCLUDED.read_bytes,
                 write_bytes = COALESCE(task_metrics.write_bytes, 0) + EXCLUDED.write_bytes,
                 samples = COALESCE(task_metrics.samples, '[]'::jsonb) || EXCLUDED.samples::jsonb"#,
        )
        .bind(&id)
        .bind(run_id)
        .bind(&sample.task_id)
        .bind(&sample.task_name)
        .bind(sample.timestamp)
        .bind(cpu_requested)
        .bind(sample.cpu_pct)
        .bind(memory_requested_mb)
        .bind(sample.memory_mb)
        .bind(sample.read_bytes)
        .bind(sample.write_bytes)
        .bind(executor)
        .bind(node_hostname)
        .bind(sample_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Called when a task completes: set finished_at, wall_seconds, exit_code.
    pub async fn finalize_task(
        &self,
        run_id: &str,
        task_id: &str,
        finished_at: DateTime<Utc>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE task_metrics SET finished_at = $1, wall_seconds = EXTRACT(EPOCH FROM ($1 - started_at))::INTEGER, exit_code = $2 WHERE run_id = $3 AND task_id = $4"#,
        )
        .bind(finished_at)
        .bind(exit_code)
        .bind(run_id)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert or replace task_metrics from Slurm sacct (one row per job/task).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_task_metrics(
        &self,
        run_id: &str,
        task_id: &str,
        task_name: &str,
        started_at: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
        wall_seconds: Option<i32>,
        cpu_requested: Option<f64>,
        cpu_peak_pct: Option<f64>,
        memory_requested_mb: Option<i64>,
        memory_peak_mb: Option<i64>,
        read_bytes: Option<i64>,
        write_bytes: Option<i64>,
        exit_code: Option<i32>,
        executor: &str,
        node_hostname: Option<&str>,
    ) -> Result<()> {
        let id = ulid::Ulid::new().to_string();
        sqlx::query(
            r#"INSERT INTO task_metrics (id, run_id, task_id, task_name, started_at, finished_at, wall_seconds, cpu_requested, cpu_peak_pct, memory_requested_mb, memory_peak_mb, read_bytes, write_bytes, exit_code, executor, node_hostname)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
        )
        .bind(&id)
        .bind(run_id)
        .bind(task_id)
        .bind(task_name)
        .bind(started_at)
        .bind(finished_at)
        .bind(wall_seconds)
        .bind(cpu_requested)
        .bind(cpu_peak_pct)
        .bind(memory_requested_mb)
        .bind(memory_peak_mb)
        .bind(read_bytes)
        .bind(write_bytes)
        .bind(exit_code)
        .bind(executor)
        .bind(node_hostname)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Called when entire run completes: aggregate task_metrics into run_cost_summary.
    pub async fn compute_run_summary(&self, run_id: &str) -> Result<RunCostSummary> {
        type TaskMetricsRow = (String, String, Option<i32>, Option<f64>, Option<i64>, Option<i64>, Option<i64>, Option<i32>);
        let rows: Vec<TaskMetricsRow> = sqlx::query_as(
            r#"SELECT task_id, task_name, wall_seconds, cpu_requested, memory_peak_mb, read_bytes, write_bytes, exit_code
               FROM task_metrics WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let snapshot = PricingSnapshot::from_config(&self.pricing);
        let mut total_wall: i64 = 0;
        let mut total_cpu_s: f64 = 0.0;
        let mut total_memory_gb_h: f64 = 0.0;
        let mut peak_memory_mb: i64 = 0;
        let mut total_read_gb: f64 = 0.0;
        let mut total_write_gb: f64 = 0.0;
        let mut breakdown = Vec::new();

        for (task_id, task_name, wall_seconds, cpu_requested, memory_peak_mb, read_bytes, write_bytes, exit_code) in
            rows
        {
            let wall = wall_seconds.unwrap_or(0) as i64;
            let cpu_req = cpu_requested.unwrap_or(1.0);
            let mem_peak = memory_peak_mb.unwrap_or(0);
            let cpu_seconds = wall as f64 * cpu_req;
            let memory_gb_h = (wall as f64 / 3600.0) * (mem_peak as f64 / 1024.0);
            total_wall += wall;
            total_cpu_s += cpu_seconds;
            total_memory_gb_h += memory_gb_h;
            peak_memory_mb = peak_memory_mb.max(mem_peak);
            total_read_gb += read_bytes.unwrap_or(0) as f64 / 1e9;
            total_write_gb += write_bytes.unwrap_or(0) as f64 / 1e9;
            let cost = snapshot.cost(cpu_seconds, memory_gb_h, None);
            breakdown.push(TaskCostBreakdown {
                task_id,
                task_name,
                wall_seconds: wall,
                cpu_seconds,
                memory_gb_h,
                peak_memory_mb: mem_peak,
                estimated_cost: cost,
                exit_code,
            });
        }

        let estimated_cost_usd = if self.pricing.enabled {
            snapshot.cost(total_cpu_s, total_memory_gb_h, None)
        } else {
            0.0
        };

        let summary = RunCostSummary {
            run_id: run_id.to_string(),
            total_wall_seconds: total_wall,
            total_cpu_seconds: total_cpu_s,
            total_memory_gb_h,
            peak_memory_mb,
            total_read_gb,
            total_write_gb,
            estimated_cost_usd,
            breakdown,
        };

        let snapshot_json = serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null);
        sqlx::query(
            r#"INSERT INTO run_cost_summary (run_id, total_wall_seconds, total_cpu_seconds, total_memory_gb_h, peak_memory_mb, total_read_gb, total_write_gb, estimated_cost_usd, pricing_config_snapshot)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (run_id) DO UPDATE SET total_wall_seconds = $2, total_cpu_seconds = $3, total_memory_gb_h = $4, peak_memory_mb = $5, total_read_gb = $6, total_write_gb = $7, estimated_cost_usd = $8, pricing_config_snapshot = $9, computed_at = now()"#,
        )
        .bind(run_id)
        .bind(summary.total_wall_seconds)
        .bind(summary.total_cpu_seconds)
        .bind(summary.total_memory_gb_h)
        .bind(summary.peak_memory_mb)
        .bind(summary.total_read_gb)
        .bind(summary.total_write_gb)
        .bind(summary.estimated_cost_usd)
        .bind(snapshot_json)
        .execute(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Estimate cost before run from workflow_engine_params (e.g. max_cores, max_memory_gb, estimated_hours).
    pub fn estimate_cost(&self, params: &serde_json::Value) -> Result<CostEstimate> {
        let snapshot = PricingSnapshot::from_config(&self.pricing);
        if !snapshot.enabled {
            return Ok(CostEstimate {
                currency: snapshot.currency,
                low_usd: 0.0,
                high_usd: None,
                assumptions: "Pricing disabled".to_string(),
            });
        }
        let empty = serde_json::Map::new();
        let obj = params.as_object().unwrap_or(&empty);
        let max_cores = obj
            .get("max_cores")
            .or(obj.get("maxCores"))
            .and_then(|v| v.as_f64())
            .unwrap_or(4.0);
        let max_memory_gb = obj
            .get("max_memory_gb")
            .or(obj.get("maxMemoryGb"))
            .and_then(|v| v.as_f64())
            .unwrap_or(16.0);
        let estimated_hours = obj
            .get("estimated_hours")
            .or(obj.get("estimatedHours"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let cpu_h = max_cores * estimated_hours;
        let memory_gb_h = max_memory_gb * estimated_hours;
        let low_usd = snapshot.cost(cpu_h * 3600.0, memory_gb_h, None);
        let high_usd = Some(low_usd * 1.5); // rough range
        Ok(CostEstimate {
            currency: snapshot.currency,
            low_usd,
            high_usd,
            assumptions: format!(
                "max_cores={}, max_memory_gb={}, estimated_hours={}",
                max_cores, max_memory_gb, estimated_hours
            ),
        })
    }

    pub fn pricing_snapshot(&self) -> PricingSnapshot {
        PricingSnapshot::from_config(&self.pricing)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Fetch run_cost_summary if present.
    pub async fn get_run_cost_summary(
        &self,
        run_id: &str,
    ) -> Result<Option<(
        i64,
        f64,
        f64,
        i64,
        f64,
        f64,
        f64,
        Option<serde_json::Value>,
    )>> {
        let row = sqlx::query_as::<_, (
            i64,
            f64,
            f64,
            i64,
            f64,
            f64,
            f64,
            Option<serde_json::Value>,
        )>(
            r#"SELECT total_wall_seconds, total_cpu_seconds, total_memory_gb_h, peak_memory_mb,
                      total_read_gb, total_write_gb, estimated_cost_usd, pricing_config_snapshot
               FROM run_cost_summary WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Task metrics row: task_id, task_name, wall_seconds, cpu_peak_pct, memory_peak_mb, exit_code, samples.
    pub async fn get_task_metrics_for_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<(
        String,
        String,
        Option<i32>,
        Option<f64>,
        Option<i64>,
        Option<i32>,
        Option<serde_json::Value>,
    )>> {
        let rows = sqlx::query_as(
            r#"SELECT task_id, task_name, wall_seconds, cpu_peak_pct, memory_peak_mb, exit_code, samples
               FROM task_metrics WHERE run_id = $1 ORDER BY started_at NULLS LAST"#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get cost for a single run (from run_cost_summary).
    pub async fn get_run_cost_usd(&self, run_id: &str) -> Result<Option<f64>> {
        let row: Option<(f64,)> =
            sqlx::query_as("SELECT estimated_cost_usd FROM run_cost_summary WHERE run_id = $1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }
}
