use crate::error::Result;
use sqlx::PgPool;

pub struct BeaconRepo {
    pool: PgPool,
}

impl BeaconRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn chromosome_candidates(chromosome: &str) -> Vec<String> {
        // HelixTest semantics use raw referenceName like "1" and our internal normalization
        // may store chromosomes either as "chr1" or "1" depending on seed/import.
        // Best-effort: try both.
        let mut out = vec![chromosome.to_string()];
        if let Some(tail) = chromosome.strip_prefix("chr") {
            if !tail.is_empty() {
                out.push(tail.to_string());
            }
        } else {
            out.push(format!("chr{chromosome}"));
        }
        out.sort();
        out.dedup();
        out
    }

    pub async fn variant_exists(
        &self,
        dataset_id: &str,
        chromosome: &str,
        start: i64,
        end: i64,
        reference: Option<&str>,
        alternate: Option<&str>,
    ) -> Result<bool> {
        let candidates = Self::chromosome_candidates(chromosome);

        // 1) Prefer exact reference/alternate match when provided.
        if let (Some(reference), Some(alternate)) = (reference, alternate) {
            let row: (bool,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM beacon_variants \
                 WHERE dataset_id = $1 \
                 AND chromosome = ANY($2) \
                 AND start <= $3 \
                 AND \"end\" >= $4 \
                 AND reference = $5 \
                 AND alternate = $6 \
                 LIMIT 1)",
            )
            .bind(dataset_id)
            .bind(&candidates)
            .bind(end)
            .bind(start)
            .bind(reference)
            .bind(alternate)
            .fetch_one(&self.pool)
            .await?;

            if row.0 {
                return Ok(true);
            }
        }

        // 2) Fallback: coordinate-only match (best-effort conformance).
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM beacon_variants \
             WHERE dataset_id = $1 \
             AND chromosome = ANY($2) \
             AND start <= $3 \
             AND \"end\" >= $4 \
             LIMIT 1)",
        )
        .bind(dataset_id)
        .bind(&candidates)
        .bind(end)
        .bind(start)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn variant_count(
        &self,
        dataset_id: &str,
        chromosome: &str,
        start: i64,
        end: i64,
        reference: Option<&str>,
        alternate: Option<&str>,
    ) -> Result<i64> {
        let candidates = Self::chromosome_candidates(chromosome);

        // 1) Exact match for allele columns when provided.
        if let (Some(reference), Some(alternate)) = (reference, alternate) {
            let row: (i64,) = sqlx::query_as(
                "SELECT COUNT(*)::bigint FROM beacon_variants \
                 WHERE dataset_id = $1 \
                 AND chromosome = ANY($2) \
                 AND start <= $3 \
                 AND \"end\" >= $4 \
                 AND reference = $5 \
                 AND alternate = $6",
            )
            .bind(dataset_id)
            .bind(&candidates)
            .bind(end)
            .bind(start)
            .bind(reference)
            .bind(alternate)
            .fetch_one(&self.pool)
            .await?;

            if row.0 > 0 {
                return Ok(row.0);
            }
        }

        // 2) Fallback coordinate-only count.
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM beacon_variants \
             WHERE dataset_id = $1 \
             AND chromosome = ANY($2) \
             AND start <= $3 \
             AND \"end\" >= $4",
        )
        .bind(dataset_id)
        .bind(&candidates)
        .bind(end)
        .bind(start)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Resolve dataset id for a given assembly_id.
    pub async fn dataset_id_for_assembly(&self, assembly_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM beacon_datasets WHERE assembly_id = $1 LIMIT 1",
        )
        .bind(assembly_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn list_datasets(&self) -> Result<Vec<(String, Option<String>, Option<String>)>> {
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
            "SELECT id, name, assembly_id FROM beacon_datasets ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
