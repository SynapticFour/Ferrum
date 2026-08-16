// SPDX-License-Identifier: BUSL-1.1
//! Minimal VCF → Beacon variant indexing (SNV rows only, capped for publish-time use).

use ferrum_core::FerrumPool;
use ferrum_core::Result;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

const MAX_VARIANT_ROWS: usize = 10_000;
const MAX_VCF_BYTES: usize = 50 * 1024 * 1024;
const INSERT_BATCH: usize = 256;

struct VcfSnv {
    chrom: String,
    pos: i64,
    reference: String,
    alternate: String,
}

/// Returns true when the object name or MIME type suggests VCF content.
pub fn is_vcf_object(name: Option<&str>, mime: Option<&str>) -> bool {
    if let Some(m) = mime {
        let ml = m.to_ascii_lowercase();
        if ml.contains("vcf") || ml.contains("vnd.ga4gh.vcf") {
            return true;
        }
    }
    name.is_some_and(|n| {
        let nl = n.to_ascii_lowercase();
        nl.ends_with(".vcf") || nl.ends_with(".vcf.gz") || nl.ends_with(".vcf.bgz")
    })
}

fn parse_vcf_snvs_from_reader(reader: impl Read) -> Result<Vec<VcfSnv>> {
    let mut peek = reader.take(MAX_VCF_BYTES as u64);
    let mut magic = [0u8; 2];
    let n = peek
        .read(&mut magic)
        .map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))?;
    let chained = Cursor::new(magic[..n].to_vec()).chain(peek);
    let decoded: Box<dyn Read> = if n == 2 && magic == [0x1f, 0x8b] {
        Box::new(flate2::read::GzDecoder::new(chained))
    } else {
        Box::new(chained)
    };
    parse_vcf_snv_lines(decoded)
}

fn parse_vcf_snvs(bytes: &[u8]) -> Result<Vec<VcfSnv>> {
    parse_vcf_snvs_from_reader(Cursor::new(bytes))
}

fn parse_vcf_snv_lines(reader: impl Read) -> Result<Vec<VcfSnv>> {
    let mut rows = Vec::new();
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let pos: i64 = match cols[1].parse() {
            Ok(p) if p > 0 => p,
            _ => continue,
        };
        let alternate = cols[4].split(',').next().unwrap_or("");
        if alternate.is_empty() || alternate == "." {
            continue;
        }
        rows.push(VcfSnv {
            chrom: cols[0].to_string(),
            pos,
            reference: cols[3].to_string(),
            alternate: alternate.to_string(),
        });
        if rows.len() >= MAX_VARIANT_ROWS {
            break;
        }
    }
    Ok(rows)
}

/// Resolve a local DRS object path from `storage_key` (ingest/publish Edge layout).
pub fn local_object_path(storage_key: &str) -> PathBuf {
    let base = std::env::var("FERRUM_STORAGE__LOCAL_PATH")
        .or_else(|_| std::env::var("FERRUM_OBJECTS_DIR"))
        .unwrap_or_else(|_| "./data".to_string());
    Path::new(&base).join(storage_key)
}

async fn insert_snvs_batched(
    pool: &FerrumPool,
    dataset_id: &str,
    rows: &[VcfSnv],
) -> Result<usize> {
    let mut inserted = 0usize;
    for chunk in rows.chunks(INSERT_BATCH) {
        match pool {
            FerrumPool::Postgres(p) => {
                let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                    "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type) ",
                );
                qb.push_values(chunk, |mut b, row| {
                    b.push_bind(dataset_id)
                        .push_bind(&row.chrom)
                        .push_bind(row.pos)
                        .push_bind(row.pos)
                        .push_bind(&row.reference)
                        .push_bind(&row.alternate)
                        .push_bind("SNV");
                });
                qb.build().execute(p).await?;
            }
            FerrumPool::Sqlite(p) => {
                let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                    "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type) ",
                );
                qb.push_values(chunk, |mut b, row| {
                    b.push_bind(dataset_id)
                        .push_bind(&row.chrom)
                        .push_bind(row.pos)
                        .push_bind(row.pos)
                        .push_bind(&row.reference)
                        .push_bind(&row.alternate)
                        .push_bind("SNV");
                });
                qb.build().execute(p).await?;
            }
        }
        inserted += chunk.len();
    }
    Ok(inserted)
}

/// Stream-parse a local VCF (plain or gzip) and insert SNV rows. Does not load the file into a `Vec`.
pub async fn index_vcf_path(pool: &FerrumPool, dataset_id: &str, path: &Path) -> Result<usize> {
    let path = path.to_path_buf();
    let rows = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)
            .map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))?;
        parse_vcf_snvs_from_reader(std::io::BufReader::new(file))
    })
    .await
    .map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))??;
    insert_snvs_batched(pool, dataset_id, &rows).await
}

/// Parse VCF bytes and insert SNV rows into `beacon_variants` (best-effort).
pub async fn index_vcf_bytes(pool: &FerrumPool, dataset_id: &str, bytes: &[u8]) -> Result<usize> {
    if bytes.len() > MAX_VCF_BYTES {
        return Ok(0);
    }
    let owned = bytes.to_vec();
    let rows = tokio::task::spawn_blocking(move || parse_vcf_snvs(&owned))
        .await
        .map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))??;
    insert_snvs_batched(pool, dataset_id, &rows).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vcf_objects() {
        assert!(is_vcf_object(Some("cohort.vcf"), None));
        assert!(is_vcf_object(Some("cohort.vcf.gz"), None));
        assert!(is_vcf_object(None, Some("application/vnd.ga4gh.vcf")));
        assert!(!is_vcf_object(Some("reads.bam"), None));
    }

    #[test]
    fn parses_snv_rows() {
        let vcf = b"##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\nchr1\t100\t.\tA\tG\n";
        let rows = parse_vcf_snvs(vcf).expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chrom, "chr1");
        assert_eq!(rows[0].pos, 100);
    }
}
