use crate::error::Result;
use sqlx::PgPool;

pub struct BeaconRepo {
    pool: PgPool,
}

impl BeaconRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn variant_exists(
        &self,
        dataset_id: &str,
        chromosome: &str,
        start: i64,
        end: i64,
    ) -> Result<bool> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM beacon_variants WHERE dataset_id = $1 AND chromosome = $2 AND start <= $3 AND \"end\" >= $4 LIMIT 1)",
        )
        .bind(dataset_id)
        .bind(chromosome)
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
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM beacon_variants WHERE dataset_id = $1 AND chromosome = $2 AND start <= $3 AND \"end\" >= $4",
        )
        .bind(dataset_id)
        .bind(chromosome)
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
