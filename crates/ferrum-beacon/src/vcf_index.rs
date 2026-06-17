//! Minimal VCF → Beacon variant indexing (SNV rows only, capped for publish-time use).

use ferrum_core::FerrumPool;
use ferrum_core::Result;
use std::io::{BufRead, BufReader, Cursor, Read};

const MAX_VARIANT_ROWS: usize = 10_000;
const MAX_VCF_BYTES: usize = 50 * 1024 * 1024;

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

fn parse_vcf_snvs(bytes: &[u8]) -> Result<Vec<VcfSnv>> {
    let reader: Box<dyn Read> = if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        Box::new(flate2::read::GzDecoder::new(Cursor::new(bytes)))
    } else {
        Box::new(Cursor::new(bytes))
    };
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

/// Parse VCF bytes and insert SNV rows into `beacon_variants` (best-effort).
pub async fn index_vcf_bytes(pool: &FerrumPool, dataset_id: &str, bytes: &[u8]) -> Result<usize> {
    if bytes.len() > MAX_VCF_BYTES {
        return Ok(0);
    }
    let owned = bytes.to_vec();
    let rows = tokio::task::spawn_blocking(move || parse_vcf_snvs(&owned))
        .await
        .map_err(|e| ferrum_core::FerrumError::Internal(anyhow::anyhow!(e)))??;

    let mut inserted = 0usize;
    for row in rows {
        let end = row.pos;
        match pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(
                    "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
                     VALUES ($1, $2, $3, $4, $5, $6, 'SNV')",
                )
                .bind(dataset_id)
                .bind(&row.chrom)
                .bind(row.pos)
                .bind(end)
                .bind(&row.reference)
                .bind(&row.alternate)
                .execute(p)
                .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'SNV')",
                )
                .bind(dataset_id)
                .bind(&row.chrom)
                .bind(row.pos)
                .bind(end)
                .bind(&row.reference)
                .bind(&row.alternate)
                .execute(p)
                .await?;
            }
        }
        inserted += 1;
    }
    Ok(inserted)
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
