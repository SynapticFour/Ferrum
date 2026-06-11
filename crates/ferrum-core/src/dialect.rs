//! Portable SQL fragments for PostgreSQL and SQLite.

use crate::pool::DbDialect;

/// Current timestamp expression.
pub fn now(d: DbDialect) -> &'static str {
    match d {
        DbDialect::Postgres => "NOW()",
        DbDialect::Sqlite => "datetime('now')",
    }
}

/// Empty JSON array default for aliases / headers columns.
pub fn empty_json_array(d: DbDialect) -> &'static str {
    match d {
        DbDialect::Postgres => "'[]'::jsonb",
        DbDialect::Sqlite => "'[]'",
    }
}

/// Lookup DRS object id by alias value.
pub fn sql_alias_lookup(d: DbDialect) -> &'static str {
    match d {
        DbDialect::Postgres => {
            "SELECT id FROM drs_objects WHERE aliases @> jsonb_build_array($1::text) LIMIT 1"
        }
        DbDialect::Sqlite => {
            "SELECT id FROM drs_objects WHERE EXISTS (SELECT 1 FROM json_each(aliases) WHERE value = $1) LIMIT 1"
        }
    }
}

/// INSERT drs_objects with dialect-specific JSON default.
pub fn sql_insert_drs_object(d: DbDialect) -> String {
    let empty = empty_json_array(d);
    let null_json = match d {
        DbDialect::Postgres => "NULL::jsonb",
        DbDialect::Sqlite => "NULL",
    };
    let is_bundle_false = match d {
        DbDialect::Postgres => "FALSE",
        DbDialect::Sqlite => "0",
    };
    format!(
        "INSERT INTO drs_objects (id, name, description, version, mime_type, size, is_bundle, aliases, workspace_id, ont_metrics)
         VALUES ($1, $2, $3, NULL, $4, $5, {is_bundle_false}, COALESCE($6, {empty}), $7, COALESCE($8, {null_json}))"
    )
}

/// Count pathogen annotations matching optional filters.
pub fn sql_pathogen_count(d: DbDialect) -> String {
    let count_expr = match d {
        DbDialect::Postgres => "COUNT(*)::bigint",
        DbDialect::Sqlite => "COUNT(*)",
    };
    let amr_contains = match d {
        DbDialect::Postgres => "($2::text IS NULL OR amr_genes @> to_jsonb(ARRAY[$2::text]))",
        DbDialect::Sqlite => {
            "($2 IS NULL OR EXISTS (SELECT 1 FROM json_each(amr_genes) WHERE value = $2))"
        }
    };
    let serotype_clause = match d {
        DbDialect::Postgres => "($3::text IS NULL OR serotype = $3)",
        DbDialect::Sqlite => "($3 IS NULL OR serotype = $3)",
    };
    let qscore_clause = match d {
        DbDialect::Postgres => "($4::double precision IS NULL OR ont_qscore_min >= $4)",
        DbDialect::Sqlite => "($4 IS NULL OR ont_qscore_min >= $4)",
    };
    format!(
        "SELECT {count_expr} FROM pathogen_annotations
         WHERE ($1::text IS NULL OR organism = $1)
           AND {amr_contains}
           AND {serotype_clause}
           AND {qscore_clause}"
    )
}

/// Pathogen existence check (returns boolean).
pub fn sql_pathogen_exists(d: DbDialect) -> String {
    let amr_contains = match d {
        DbDialect::Postgres => "($2::text IS NULL OR amr_genes @> to_jsonb(ARRAY[$2::text]))",
        DbDialect::Sqlite => {
            "($2 IS NULL OR EXISTS (SELECT 1 FROM json_each(amr_genes) WHERE value = $2))"
        }
    };
    let serotype_clause = match d {
        DbDialect::Postgres => "($3::text IS NULL OR serotype = $3)",
        DbDialect::Sqlite => "($3 IS NULL OR serotype = $3)",
    };
    let qscore_clause = match d {
        DbDialect::Postgres => "($4::double precision IS NULL OR ont_qscore_min >= $4)",
        DbDialect::Sqlite => "($4 IS NULL OR ont_qscore_min >= $4)",
    };
    format!(
        "SELECT EXISTS (
            SELECT 1 FROM pathogen_annotations
            WHERE ($1::text IS NULL OR organism = $1)
              AND {amr_contains}
              AND {serotype_clause}
              AND {qscore_clause}
         )"
    )
}

/// List DRS objects with optional filters.
pub fn sql_list_objects(d: DbDialect) -> String {
    let null_text = match d {
        DbDialect::Postgres => "$1::text IS NULL",
        DbDialect::Sqlite => "$1 IS NULL",
    };
    let null_bigint = |n: &str| match d {
        DbDialect::Postgres => format!("${n}::bigint IS NULL"),
        DbDialect::Sqlite => format!("${n} IS NULL"),
    };
    let null_ws = match d {
        DbDialect::Postgres => "$6::text IS NULL",
        DbDialect::Sqlite => "$6 IS NULL",
    };
    format!(
        "SELECT id, name, description, created_time, updated_time, version, mime_type, size, is_bundle, aliases, dataset_id
         FROM drs_objects
         WHERE ({null_text} OR mime_type = $1)
           AND ({min} OR size >= $2)
           AND ({max} OR size <= $3)
           AND ({null_ws} OR workspace_id = $6)
         ORDER BY created_time DESC LIMIT $4 OFFSET $5",
        min = null_bigint("2"),
        max = null_bigint("3"),
    )
}

/// INSERT drs_access_methods with empty headers default.
pub fn sql_insert_access_method(d: DbDialect) -> String {
    let empty = empty_json_array(d);
    format!(
        "INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers) VALUES ($1, 'https', $2, $3, {empty})"
    )
}

/// UPDATE drs_objects admin patch.
pub fn sql_update_drs_object(d: DbDialect) -> String {
    let now = now(d);
    format!(
        "UPDATE drs_objects SET updated_time = {now}, name = COALESCE($2, name), description = COALESCE($3, description),
         mime_type = COALESCE($4, mime_type), size = COALESCE($5, size), aliases = COALESCE($6, aliases) WHERE id = $1"
    )
}

/// UPDATE ingest job success.
pub fn sql_ingest_job_succeeded(d: DbDialect) -> String {
    let now = now(d);
    format!(
        "UPDATE drs_ingest_jobs SET status = 'succeeded', result_json = $2, error_json = NULL, updated_at = {now} WHERE id = $1"
    )
}

/// Paginated bundle contents listing.
pub fn sql_list_bundle_contents_page(d: DbDialect) -> String {
    let null_cursor = match d {
        DbDialect::Postgres => "$2::text IS NULL",
        DbDialect::Sqlite => "$2 IS NULL",
    };
    let cmp = match d {
        DbDialect::Postgres => "c.object_id > $2::text",
        DbDialect::Sqlite => "c.object_id > $2",
    };
    format!(
        "SELECT c.object_id, c.name, c.drs_uri
         FROM drs_bundle_contents c
         WHERE c.bundle_id = $1
           AND ({null_cursor} OR {cmp})
         ORDER BY c.object_id
         LIMIT $3"
    )
}

/// UPDATE ingest job failed.
pub fn sql_ingest_job_failed(d: DbDialect) -> String {
    let now = now(d);
    format!(
        "UPDATE drs_ingest_jobs SET status = 'failed', error_json = $2, updated_at = {now} WHERE id = $1"
    )
}

/// Beacon variant exists — coordinate match with optional alleles.
pub fn sql_beacon_variant_exists_exact(d: DbDialect) -> String {
    let chrom = chromosome_in_clause(d, "$2");
    format!(
        "SELECT EXISTS(SELECT 1 FROM beacon_variants \
         WHERE dataset_id = $1 \
         AND {chrom} \
         AND start <= $3 \
         AND \"end\" >= $4 \
         AND reference = $5 \
         AND alternate = $6 \
         LIMIT 1)"
    )
}

pub fn sql_beacon_variant_exists_coord(d: DbDialect) -> String {
    let chrom = chromosome_in_clause(d, "$2");
    format!(
        "SELECT EXISTS(SELECT 1 FROM beacon_variants \
         WHERE dataset_id = $1 \
         AND {chrom} \
         AND start <= $3 \
         AND \"end\" >= $4 \
         LIMIT 1)"
    )
}

pub fn sql_beacon_variant_count_exact(d: DbDialect) -> String {
    let chrom = chromosome_in_clause(d, "$2");
    let count = match d {
        DbDialect::Postgres => "COUNT(*)::bigint",
        DbDialect::Sqlite => "COUNT(*)",
    };
    format!(
        "SELECT {count} FROM beacon_variants \
         WHERE dataset_id = $1 \
         AND {chrom} \
         AND start <= $3 \
         AND \"end\" >= $4 \
         AND reference = $5 \
         AND alternate = $6"
    )
}

pub fn sql_beacon_variant_count_coord(d: DbDialect) -> String {
    let chrom = chromosome_in_clause(d, "$2");
    let count = match d {
        DbDialect::Postgres => "COUNT(*)::bigint",
        DbDialect::Sqlite => "COUNT(*)",
    };
    format!(
        "SELECT {count} FROM beacon_variants \
         WHERE dataset_id = $1 \
         AND {chrom} \
         AND start <= $3 \
         AND \"end\" >= $4"
    )
}

fn chromosome_in_clause(d: DbDialect, param: &str) -> String {
    match d {
        DbDialect::Postgres => format!("chromosome = ANY({param})"),
        DbDialect::Sqlite => format!("chromosome IN (SELECT value FROM json_each({param}))"),
    }
}

pub fn sql_beacon_variant_match_ids(d: DbDialect) -> String {
    let chrom = chromosome_in_clause(d, "$2");
    let id_cast = match d {
        DbDialect::Postgres => "id::bigint",
        DbDialect::Sqlite => "id",
    };
    let null_ref = match d {
        DbDialect::Postgres => "$5::text IS NULL",
        DbDialect::Sqlite => "$5 IS NULL",
    };
    let null_alt = match d {
        DbDialect::Postgres => "$6::text IS NULL",
        DbDialect::Sqlite => "$6 IS NULL",
    };
    let null_vt = match d {
        DbDialect::Postgres => "$7::text IS NULL",
        DbDialect::Sqlite => "$7 IS NULL",
    };
    format!(
        "SELECT {id_cast} FROM beacon_variants
         WHERE dataset_id = $1
           AND {chrom}
           AND start <= $3
           AND \"end\" >= $4
           AND ({null_ref} OR reference = $5)
           AND ({null_alt} OR alternate = $6)
           AND ({null_vt} OR variant_type = $7)"
    )
}

/// JSON array string for SQLite `json_each` chromosome binding.
pub fn chromosomes_json(candidates: &[String]) -> String {
    serde_json::to_string(candidates).unwrap_or_else(|_| "[]".to_string())
}
