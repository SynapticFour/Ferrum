// SPDX-License-Identifier: BUSL-1.1
use crate::error::Result;
use ferrum_core::{
    chromosomes_json, sql_beacon_variant_count_coord, sql_beacon_variant_count_exact,
    sql_beacon_variant_exists_coord, sql_beacon_variant_exists_exact, sql_beacon_variant_match_ids,
    sql_pathogen_count, sql_pathogen_exists, DbDialect, FerrumPool,
};

pub struct BeaconRepo {
    pool: FerrumPool,
    #[allow(dead_code)]
    dialect: DbDialect,
}

impl BeaconRepo {
    pub fn new(pool: FerrumPool) -> Self {
        let dialect = pool.dialect();
        Self { pool, dialect }
    }

    fn chromosome_candidates(chromosome: &str) -> Vec<String> {
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

        if let (Some(reference), Some(alternate)) = (reference, alternate) {
            let row: (bool,) = match &self.pool {
                FerrumPool::Postgres(p) => {
                    sqlx::query_as(&sql_beacon_variant_exists_exact(DbDialect::Postgres))
                        .bind(dataset_id)
                        .bind(&candidates)
                        .bind(end)
                        .bind(start)
                        .bind(reference)
                        .bind(alternate)
                        .fetch_one(p)
                        .await?
                }
                FerrumPool::Sqlite(p) => {
                    sqlx::query_as(&sql_beacon_variant_exists_exact(DbDialect::Sqlite))
                        .bind(dataset_id)
                        .bind(chromosomes_json(&candidates))
                        .bind(end)
                        .bind(start)
                        .bind(reference)
                        .bind(alternate)
                        .fetch_one(p)
                        .await?
                }
            };

            if row.0 {
                return Ok(true);
            }
        }

        let row: (bool,) = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(&sql_beacon_variant_exists_coord(DbDialect::Postgres))
                    .bind(dataset_id)
                    .bind(&candidates)
                    .bind(end)
                    .bind(start)
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(&sql_beacon_variant_exists_coord(DbDialect::Sqlite))
                    .bind(dataset_id)
                    .bind(chromosomes_json(&candidates))
                    .bind(end)
                    .bind(start)
                    .fetch_one(p)
                    .await?
            }
        };
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

        if let (Some(reference), Some(alternate)) = (reference, alternate) {
            let row: (i64,) = match &self.pool {
                FerrumPool::Postgres(p) => {
                    sqlx::query_as(&sql_beacon_variant_count_exact(DbDialect::Postgres))
                        .bind(dataset_id)
                        .bind(&candidates)
                        .bind(end)
                        .bind(start)
                        .bind(reference)
                        .bind(alternate)
                        .fetch_one(p)
                        .await?
                }
                FerrumPool::Sqlite(p) => {
                    sqlx::query_as(&sql_beacon_variant_count_exact(DbDialect::Sqlite))
                        .bind(dataset_id)
                        .bind(chromosomes_json(&candidates))
                        .bind(end)
                        .bind(start)
                        .bind(reference)
                        .bind(alternate)
                        .fetch_one(p)
                        .await?
                }
            };

            if row.0 > 0 {
                return Ok(row.0);
            }
        }

        let row: (i64,) = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(&sql_beacon_variant_count_coord(DbDialect::Postgres))
                    .bind(dataset_id)
                    .bind(&candidates)
                    .bind(end)
                    .bind(start)
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(&sql_beacon_variant_count_coord(DbDialect::Sqlite))
                    .bind(dataset_id)
                    .bind(chromosomes_json(&candidates))
                    .bind(end)
                    .bind(start)
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(row.0)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn variant_match_ids(
        &self,
        dataset_id: &str,
        chromosome: &str,
        start: i64,
        end: i64,
        reference: Option<&str>,
        alternate: Option<&str>,
        variant_type: Option<&str>,
    ) -> Result<Vec<i64>> {
        let candidates = Self::chromosome_candidates(chromosome);

        let rows: Vec<(i64,)> = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(&sql_beacon_variant_match_ids(DbDialect::Postgres))
                    .bind(dataset_id)
                    .bind(&candidates)
                    .bind(end)
                    .bind(start)
                    .bind(reference)
                    .bind(alternate)
                    .bind(variant_type)
                    .fetch_all(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(&sql_beacon_variant_match_ids(DbDialect::Sqlite))
                    .bind(dataset_id)
                    .bind(chromosomes_json(&candidates))
                    .bind(end)
                    .bind(start)
                    .bind(reference)
                    .bind(alternate)
                    .bind(variant_type)
                    .fetch_all(p)
                    .await?
            }
        };

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn dataset_id_for_assembly(&self, assembly_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as("SELECT id FROM beacon_datasets WHERE assembly_id = $1 LIMIT 1")
                    .bind(assembly_id)
                    .fetch_optional(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as("SELECT id FROM beacon_datasets WHERE assembly_id = $1 LIMIT 1")
                    .bind(assembly_id)
                    .fetch_optional(p)
                    .await?
            }
        };
        Ok(row.map(|r| r.0))
    }

    pub async fn list_datasets(&self) -> Result<Vec<(String, Option<String>, Option<String>)>> {
        let rows = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
                    "SELECT id, name, assembly_id FROM beacon_datasets ORDER BY id",
                )
                .fetch_all(p)
                .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
                    "SELECT id, name, assembly_id FROM beacon_datasets ORDER BY id",
                )
                .fetch_all(p)
                .await?
            }
        };
        Ok(rows)
    }

    /// Pathogen annotation existence (multi-organism Beacon).
    pub async fn pathogen_exists(
        &self,
        organism: Option<&str>,
        amr_gene: Option<&str>,
        serotype: Option<&str>,
        min_qscore: Option<f32>,
    ) -> Result<bool> {
        let row: (bool,) = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(&sql_pathogen_exists(DbDialect::Postgres))
                    .bind(organism)
                    .bind(amr_gene)
                    .bind(serotype)
                    .bind(min_qscore)
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(&sql_pathogen_exists(DbDialect::Sqlite))
                    .bind(organism)
                    .bind(amr_gene)
                    .bind(serotype)
                    .bind(min_qscore)
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(row.0)
    }

    /// Pathogen annotation count.
    pub async fn pathogen_count(
        &self,
        organism: Option<&str>,
        amr_gene: Option<&str>,
        serotype: Option<&str>,
        min_qscore: Option<f32>,
    ) -> Result<i64> {
        let row: (i64,) = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(&sql_pathogen_count(DbDialect::Postgres))
                    .bind(organism)
                    .bind(amr_gene)
                    .bind(serotype)
                    .bind(min_qscore)
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(&sql_pathogen_count(DbDialect::Sqlite))
                    .bind(organism)
                    .bind(amr_gene)
                    .bind(serotype)
                    .bind(min_qscore)
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(row.0)
    }

    /// Ensure a Beacon dataset row exists (idempotent).
    pub async fn ensure_dataset(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        assembly_id: &str,
    ) -> Result<()> {
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(
                    "INSERT INTO beacon_datasets (id, name, description, assembly_id)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT (id) DO UPDATE SET
                       name = EXCLUDED.name,
                       description = COALESCE(EXCLUDED.description, beacon_datasets.description)",
                )
                .bind(id)
                .bind(name)
                .bind(description)
                .bind(assembly_id)
                .execute(p)
                .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "INSERT INTO beacon_datasets (id, name, description, assembly_id)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (id) DO UPDATE SET
                       name = excluded.name,
                       description = COALESCE(excluded.description, beacon_datasets.description)",
                )
                .bind(id)
                .bind(name)
                .bind(description)
                .bind(assembly_id)
                .execute(p)
                .await?;
            }
        }
        Ok(())
    }

    /// Insert pathogen annotation (tests and ingest helpers).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_pathogen_annotation(
        &self,
        id: &str,
        organism: &str,
        amr_genes: &[String],
        serotype: Option<&str>,
        ont_qscore_min: Option<f32>,
        drs_object_id: Option<&str>,
        dataset_id: Option<&str>,
    ) -> Result<()> {
        let amr_json = serde_json::to_value(amr_genes).unwrap_or(serde_json::json!([]));
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(
                    "INSERT INTO pathogen_annotations (id, dataset_id, drs_object_id, organism, amr_genes, serotype, virulence_factors, ont_qscore_min)
                     VALUES ($1, $2, $3, $4, $5, $6, '[]', $7)",
                )
                .bind(id)
                .bind(dataset_id)
                .bind(drs_object_id)
                .bind(organism)
                .bind(amr_json)
                .bind(serotype)
                .bind(ont_qscore_min)
                .execute(p)
                .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "INSERT INTO pathogen_annotations (id, dataset_id, drs_object_id, organism, amr_genes, serotype, virulence_factors, ont_qscore_min)
                     VALUES ($1, $2, $3, $4, $5, $6, '[]', $7)",
                )
                .bind(id)
                .bind(dataset_id)
                .bind(drs_object_id)
                .bind(organism)
                .bind(amr_json)
                .bind(serotype)
                .bind(ont_qscore_min)
                .execute(p)
                .await?;
            }
        }
        Ok(())
    }
}
