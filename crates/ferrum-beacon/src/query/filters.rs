//! Beacon v2 filter evaluation helpers.
//!
//! Learned from EGA `beacon2-pi-api`: when an OR-filter spans different "collections"
//! (e.g. genomic-variation vs non-genomic-variation terms), implementations must
//! evaluate each side separately and then merge+deduplicate results. Without
//! explicit deduplication, overlapping hits can be double-counted.

use std::collections::HashSet;

/// Source collection kind for a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterCollection {
    GenomicVariation,
    Individual,
}

/// A minimal representation of a hit that could be returned by a collection query.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hit {
    /// Stable identifier used for deduplication across collections.
    pub id: String,
    /// Which collection produced this hit.
    pub collection: FilterCollection,
}

/// Merge hits from two OR-sides while deduplicating by `Hit.id`.
///
/// When both sides match the same logical entity, we keep exactly one hit to avoid
/// inflated `count` results (EGA lesson: cross-collection OR must be explicit about dedup).
pub fn merge_or_dedup_by_id(left: Vec<Hit>, right: Vec<Hit>) -> Vec<Hit> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for h in left.into_iter().chain(right.into_iter()) {
        if seen.insert(h.id.clone()) {
            out.push(h);
        }
    }

    // Deterministic ordering for stable tests.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Evaluate OR over two collection scopes and merge+deduplicate.
///
/// `eval_left` and `eval_right` are intentionally separate to mirror production logic
/// where OR spans collections that cannot be joined safely in one DB query.
pub fn eval_or_cross_collections<L, R>(eval_left: L, eval_right: R) -> Vec<Hit>
where
    L: FnOnce() -> Vec<Hit>,
    R: FnOnce() -> Vec<Hit>,
{
    let left = eval_left();
    let right = eval_right();
    merge_or_dedup_by_id(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_or_filter_cross_collection_deduplication() {
        // Two independent collection queries (genomicVariations and individuals).
        // Both match logical entity "v1", so merged results must contain v1 once.
        let genomic_hits = vec![
            Hit {
                id: "v1".to_string(),
                collection: FilterCollection::GenomicVariation,
            },
            Hit {
                id: "v2".to_string(),
                collection: FilterCollection::GenomicVariation,
            },
        ];
        let individual_hits = vec![
            Hit {
                id: "v1".to_string(),
                collection: FilterCollection::Individual,
            },
            Hit {
                id: "v3".to_string(),
                collection: FilterCollection::Individual,
            },
        ];

        let merged = merge_or_dedup_by_id(genomic_hits, individual_hits);
        let ids: Vec<String> = merged.into_iter().map(|h| h.id).collect();
        assert_eq!(ids, vec!["v1", "v2", "v3"]);
    }

    #[test]
    fn test_merge_or_dedup_is_id_based() {
        // Dedup key is logical id; collection kind does not matter.
        let left = vec![Hit {
            id: "x".into(),
            collection: FilterCollection::Individual,
        }];
        let right = vec![Hit {
            id: "x".into(),
            collection: FilterCollection::GenomicVariation,
        }];

        let merged = merge_or_dedup_by_id(left, right);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "x");
    }
}
