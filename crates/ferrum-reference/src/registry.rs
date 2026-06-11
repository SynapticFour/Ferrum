//! Reference genome registry database access.

use crate::types::{
    LoadReferenceRequest, PopulationScope, ReferenceGenome, RegisterReferenceRequest,
};
use ferrum_core::{FerrumError, FerrumPool, Result};
use url::Url;

pub struct ReferenceRegistry {
    pool: FerrumPool,
}

impl ReferenceRegistry {
    pub fn new(pool: FerrumPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &FerrumPool {
        &self.pool
    }

    pub async fn list(&self) -> Result<Vec<ReferenceGenome>> {
        let sql = "SELECT id, display_name, organism, population_scope, source_url, fasta_drs_id, index_drs_id, is_default
            FROM reference_genomes ORDER BY id";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                let rows = sqlx::query_as::<_, ReferenceRow>(sql).fetch_all(p).await?;
                Ok(rows.into_iter().map(ReferenceRow::into).collect())
            }
            FerrumPool::Sqlite(p) => {
                let rows = sqlx::query_as::<_, ReferenceRow>(sql).fetch_all(p).await?;
                Ok(rows.into_iter().map(ReferenceRow::into).collect())
            }
        }
    }

    pub async fn get(&self, id: &str) -> Result<Option<ReferenceGenome>> {
        let sql = "SELECT id, display_name, organism, population_scope, source_url, fasta_drs_id, index_drs_id, is_default
            FROM reference_genomes WHERE id = $1";
        let row = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as::<_, ReferenceRow>(sql)
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as::<_, ReferenceRow>(sql)
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
        };
        Ok(row.map(ReferenceRow::into))
    }

    pub async fn register(&self, req: &RegisterReferenceRequest) -> Result<ReferenceGenome> {
        if req.id.trim().is_empty() {
            return Err(FerrumError::ValidationError("id required".into()));
        }
        let scope = req.population_scope.to_db_string();
        let source_url = req.source_url.as_ref().map(|u| u.to_string());
        if req.is_default {
            self.clear_default().await?;
        }
        let sql = "INSERT INTO reference_genomes (id, display_name, organism, population_scope, source_url, is_default)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                display_name = EXCLUDED.display_name,
                organism = EXCLUDED.organism,
                population_scope = EXCLUDED.population_scope,
                source_url = EXCLUDED.source_url,
                is_default = EXCLUDED.is_default";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(sql)
                    .bind(&req.id)
                    .bind(&req.display_name)
                    .bind(&req.organism)
                    .bind(&scope)
                    .bind(&source_url)
                    .bind(req.is_default)
                    .execute(p)
                    .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(sql)
                    .bind(&req.id)
                    .bind(&req.display_name)
                    .bind(&req.organism)
                    .bind(&scope)
                    .bind(&source_url)
                    .bind(req.is_default)
                    .execute(p)
                    .await?;
            }
        }
        self.get(&req.id)
            .await?
            .ok_or_else(|| FerrumError::Internal(anyhow::anyhow!("reference insert failed")))
    }

    pub async fn load_fasta(
        &self,
        id: &str,
        req: &LoadReferenceRequest,
    ) -> Result<ReferenceGenome> {
        if !self.drs_object_exists(&req.fasta_drs_id).await? {
            return Err(FerrumError::ValidationError(format!(
                "unknown fasta_drs_id {}",
                req.fasta_drs_id
            )));
        }
        if let Some(ref idx) = req.index_drs_id {
            if !self.drs_object_exists(idx).await? {
                return Err(FerrumError::ValidationError(format!(
                    "unknown index_drs_id {idx}"
                )));
            }
        }
        let sql = "UPDATE reference_genomes SET fasta_drs_id = $2, index_drs_id = $3 WHERE id = $1";
        let updated = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query(sql)
                .bind(id)
                .bind(&req.fasta_drs_id)
                .bind(&req.index_drs_id)
                .execute(p)
                .await?
                .rows_affected(),
            FerrumPool::Sqlite(p) => sqlx::query(sql)
                .bind(id)
                .bind(&req.fasta_drs_id)
                .bind(&req.index_drs_id)
                .execute(p)
                .await?
                .rows_affected(),
        };
        if updated == 0 {
            return Err(FerrumError::NotFound(format!("reference {id} not found")));
        }
        self.get(id)
            .await?
            .ok_or_else(|| FerrumError::NotFound(format!("reference {id} not found")))
    }

    pub async fn default_reference(&self) -> Result<Option<ReferenceGenome>> {
        let sql = "SELECT id, display_name, organism, population_scope, source_url, fasta_drs_id, index_drs_id, is_default
            FROM reference_genomes WHERE is_default = TRUE LIMIT 1";
        let row = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as::<_, ReferenceRow>(sql)
                    .fetch_optional(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as::<_, ReferenceRow>(sql)
                    .fetch_optional(p)
                    .await?
            }
        };
        Ok(row.map(ReferenceRow::into))
    }

    pub async fn african_pangenome_alternatives(&self) -> Result<Vec<String>> {
        let all = self.list().await?;
        Ok(all
            .into_iter()
            .filter(|r| matches!(r.population_scope, PopulationScope::AfricanPangenome))
            .map(|r| r.id)
            .collect())
    }

    async fn clear_default(&self) -> Result<()> {
        let sql = "UPDATE reference_genomes SET is_default = FALSE WHERE is_default = TRUE";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(sql).execute(p).await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(sql).execute(p).await?;
            }
        }
        Ok(())
    }

    async fn drs_object_exists(&self, id: &str) -> Result<bool> {
        let sql = "SELECT 1 FROM drs_objects WHERE id = $1 LIMIT 1";
        let row: Option<i32> = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query_scalar(sql).bind(id).fetch_optional(p).await?,
            FerrumPool::Sqlite(p) => sqlx::query_scalar(sql).bind(id).fetch_optional(p).await?,
        };
        Ok(row.is_some())
    }
}

#[derive(sqlx::FromRow)]
struct ReferenceRow {
    id: String,
    display_name: String,
    organism: String,
    population_scope: String,
    source_url: Option<String>,
    fasta_drs_id: Option<String>,
    index_drs_id: Option<String>,
    is_default: bool,
}

impl ReferenceRow {
    fn into(self) -> ReferenceGenome {
        ReferenceGenome {
            id: self.id,
            display_name: self.display_name,
            organism: self.organism,
            population_scope: PopulationScope::from_db_string(&self.population_scope)
                .unwrap_or(PopulationScope::Global),
            source_url: self.source_url.and_then(|s| Url::parse(&s).ok()),
            fasta_drs_id: self.fasta_drs_id,
            index_drs_id: self.index_drs_id,
            is_default: self.is_default,
        }
    }
}
