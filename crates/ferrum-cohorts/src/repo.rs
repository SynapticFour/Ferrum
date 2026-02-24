//! Database repository for cohorts and samples.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

pub struct CohortRepo {
    pool: PgPool,
}

impl CohortRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_cohort(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        owner_sub: &str,
        workspace_id: Option<&str>,
        tags: &Value,
        filter_criteria: &Value,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO cohorts (id, name, description, owner_sub, workspace_id, tags, filter_criteria)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(owner_sub)
        .bind(workspace_id)
        .bind(tags)
        .bind(filter_criteria)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_cohort(&self, id: &str) -> Result<Option<(String, String, Option<String>, String, Option<String>, i32, bool, i32, Value, Value, DateTime<Utc>, DateTime<Utc>)>> {
        let row = sqlx::query_as(
            r#"SELECT id, name, description, owner_sub, workspace_id, version, is_frozen, sample_count, tags, filter_criteria, created_at, updated_at
               FROM cohorts WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Returns true if the caller (sub) may read/write the cohort: owner, in cohort_members, or workspace member when cohort has workspace_id.
    pub async fn cohort_accessible_by(&self, cohort_id: &str, sub: &str) -> Result<bool> {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT owner_sub, workspace_id FROM cohorts WHERE id = $1",
        )
        .bind(cohort_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some((owner_sub, workspace_id)) = row else {
            return Ok(false);
        };
        if owner_sub == sub {
            return Ok(true);
        }
        let member: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM cohort_members WHERE cohort_id = $1 AND sub = $2",
        )
        .bind(cohort_id)
        .bind(sub)
        .fetch_optional(&self.pool)
        .await?;
        if member.is_some() {
            return Ok(true);
        }
        if let Some(ref ws_id) = workspace_id {
            if ferrum_core::get_workspace_member_role(&self.pool, ws_id, sub)
                .await
                .map_err(|e| crate::error::CohortError::Other(e.into()))?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// List cohorts visible to the caller: owned by sub or where (cohort_id, sub) in cohort_members. If workspace_id is set, only cohorts in that workspace (caller must be workspace member — checked in handler).
    pub async fn list_cohorts(
        &self,
        caller_sub: Option<&str>,
        tag_filter: Option<&str>,
        workspace_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(String, String, Option<String>, String, Option<String>, i32, bool, i32, Value, DateTime<Utc>, DateTime<Utc>)>> {
        type Row = (String, String, Option<String>, String, Option<String>, i32, bool, i32, Value, DateTime<Utc>, DateTime<Utc>);
        let rows: Vec<Row> = match caller_sub {
            Some(sub) => {
                let q = if let Some(ws_id) = workspace_id {
                    sqlx::query_as(
                        r#"SELECT c.id, c.name, c.description, c.owner_sub, c.workspace_id, c.version, c.is_frozen, c.sample_count, c.tags, c.created_at, c.updated_at
                           FROM cohorts c
                           LEFT JOIN cohort_members m ON c.id = m.cohort_id AND m.sub = $1
                           WHERE (c.owner_sub = $1 OR m.sub IS NOT NULL) AND c.workspace_id = $4
                           ORDER BY c.updated_at DESC LIMIT $2 OFFSET $3"#,
                    )
                    .bind(sub)
                    .bind(limit)
                    .bind(offset)
                    .bind(ws_id)
                } else {
                    sqlx::query_as(
                        r#"SELECT c.id, c.name, c.description, c.owner_sub, c.workspace_id, c.version, c.is_frozen, c.sample_count, c.tags, c.created_at, c.updated_at
                           FROM cohorts c
                           LEFT JOIN cohort_members m ON c.id = m.cohort_id AND m.sub = $1
                           WHERE c.owner_sub = $1 OR m.sub IS NOT NULL
                           ORDER BY c.updated_at DESC LIMIT $2 OFFSET $3"#,
                    )
                    .bind(sub)
                    .bind(limit)
                    .bind(offset)
                };
                q.fetch_all(&self.pool).await?
            }
            None => return Ok(Vec::new()),
        };
        let filtered: Vec<Row> = if let Some(tag) = tag_filter {
            rows.into_iter()
                .filter(|r| {
                    let tags = &r.8;
                    tags.as_array()
                        .map(|a| a.iter().any(|v| v.as_str() == Some(tag)))
                        .unwrap_or(false)
                })
                .collect()
        } else {
            rows
        };
        Ok(filtered)
    }

    pub async fn update_cohort(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        tags: Option<&Value>,
        filter_criteria: Option<&Value>,
    ) -> Result<()> {
        let row = self.get_cohort(id).await?.ok_or_else(|| crate::error::CohortError::NotFound(id.to_string()))?;
        let (_, name_cur, desc_cur, _, _, _, frozen, _, tags_cur, criteria_cur, _, _) = row;
        if frozen {
            return Err(crate::error::CohortError::Validation("cohort is frozen".into()));
        }
        let name = name.unwrap_or(&name_cur);
        let description = description.or(desc_cur.as_deref());
        let tags = tags.unwrap_or(&tags_cur);
        let filter_criteria = filter_criteria.unwrap_or(&criteria_cur);
        sqlx::query(
            r#"UPDATE cohorts SET name = $1, description = $2, tags = $3, filter_criteria = $4, updated_at = now() WHERE id = $5"#,
        )
        .bind(name)
        .bind(description)
        .bind(tags)
        .bind(filter_criteria)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_cohort(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM cohorts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn freeze_cohort(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE cohorts SET is_frozen = true, updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_sample_count(&self, cohort_id: &str, count: i32) -> Result<()> {
        sqlx::query("UPDATE cohorts SET sample_count = $1, updated_at = now() WHERE id = $2")
            .bind(count)
            .bind(cohort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- cohort_samples ---
    pub async fn add_sample(
        &self,
        id: &str,
        cohort_id: &str,
        sample_id: &str,
        drs_object_ids: &Value,
        phenotype: &Value,
        added_by: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO cohort_samples (id, cohort_id, sample_id, drs_object_ids, phenotype, added_by)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (cohort_id, sample_id) DO UPDATE SET drs_object_ids = $4, phenotype = $5, added_by = $6"#,
        )
        .bind(id)
        .bind(cohort_id)
        .bind(sample_id)
        .bind(drs_object_ids)
        .bind(phenotype)
        .bind(added_by)
        .execute(&self.pool)
        .await?;
        self.recount_samples(cohort_id).await?;
        Ok(())
    }

    pub async fn get_sample(
        &self,
        cohort_id: &str,
        sample_id: &str,
    ) -> Result<Option<(String, String, String, Value, Value, DateTime<Utc>, String)>> {
        let row = sqlx::query_as(
            r#"SELECT id, cohort_id, sample_id, drs_object_ids, phenotype, added_at, added_by FROM cohort_samples WHERE cohort_id = $1 AND sample_id = $2"#,
        )
        .bind(cohort_id)
        .bind(sample_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_samples(
        &self,
        cohort_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(String, String, String, Value, Value, DateTime<Utc>, String)>> {
        let rows = sqlx::query_as(
            r#"SELECT id, cohort_id, sample_id, drs_object_ids, phenotype, added_at, added_by
               FROM cohort_samples WHERE cohort_id = $1 ORDER BY added_at DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(cohort_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_sample(
        &self,
        cohort_id: &str,
        sample_id: &str,
        drs_object_ids: Option<&Value>,
        phenotype: Option<&Value>,
    ) -> Result<()> {
        let cur = self
            .get_sample(cohort_id, sample_id)
            .await?
            .ok_or_else(|| crate::error::CohortError::NotFound(format!("sample {} in cohort", sample_id)))?;
        let (_, _, _, drs_cur, pheno_cur, _, _) = cur;
        let drs = drs_object_ids.unwrap_or(&drs_cur);
        let pheno = phenotype.unwrap_or(&pheno_cur);
        sqlx::query(
            "UPDATE cohort_samples SET drs_object_ids = $1, phenotype = $2 WHERE cohort_id = $3 AND sample_id = $4",
        )
        .bind(drs)
        .bind(pheno)
        .bind(cohort_id)
        .bind(sample_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_sample(&self, cohort_id: &str, sample_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM cohort_samples WHERE cohort_id = $1 AND sample_id = $2")
            .bind(cohort_id)
            .bind(sample_id)
            .execute(&self.pool)
            .await?;
        self.recount_samples(cohort_id).await?;
        Ok(())
    }

    pub async fn samples_for_cohort(&self, cohort_id: &str) -> Result<Vec<(String, Value, Value)>> {
        let rows = sqlx::query_as::<_, (String, Value, Value)>(
            "SELECT sample_id, drs_object_ids, phenotype FROM cohort_samples WHERE cohort_id = $1",
        )
        .bind(cohort_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn recount_samples(&self, cohort_id: &str) -> Result<()> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM cohort_samples WHERE cohort_id = $1")
            .bind(cohort_id)
            .fetch_one(&self.pool)
            .await?;
        self.set_sample_count(cohort_id, count as i32).await?;
        Ok(())
    }

    // --- phenotype_schema ---
    pub async fn list_phenotype_schema(
        &self,
    ) -> Result<Vec<(String, String, String, String, Option<String>, bool, Option<String>)>> {
        let rows = sqlx::query_as(
            "SELECT id, field_name, display_name, field_type, ontology, required, description FROM phenotype_schema ORDER BY field_name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- cohort_versions ---
    pub async fn create_version(
        &self,
        id: &str,
        cohort_id: &str,
        version: i32,
        snapshot: &Value,
        created_by: &str,
        note: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO cohort_versions (id, cohort_id, version, snapshot, created_by, note) VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(cohort_id)
        .bind(version)
        .bind(snapshot)
        .bind(created_by)
        .bind(note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_versions(&self, cohort_id: &str) -> Result<Vec<(String, i32, DateTime<Utc>, String, Option<String>)>> {
        let rows = sqlx::query_as(
            "SELECT id, version, created_at, created_by, note FROM cohort_versions WHERE cohort_id = $1 ORDER BY version DESC",
        )
        .bind(cohort_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
