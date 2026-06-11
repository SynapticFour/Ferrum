//! Multi-pathogen Beacon filters (`PathoGenFilter` + requestParameters extensions).

use crate::handlers::{BeaconFilter, BeaconFilterExpr, PathogenFilterParams};

/// Merge pathogen constraints from `requestParameters` and Beacon v2 `filters` (including
/// `{ "id": "PathoGenFilter", "organism": "...", ... }` entries).
pub fn merge_pathogen_params(
    mut base: PathogenFilterParams,
    filters: &[BeaconFilterExpr],
) -> PathogenFilterParams {
    for expr in filters {
        if let BeaconFilterExpr::Single(f) = expr {
            merge_from_beacon_filter(&mut base, f);
        } else if let BeaconFilterExpr::OrGroup(group) = expr {
            for f in group {
                merge_from_beacon_filter(&mut base, f);
            }
        }
    }
    base
}

fn merge_from_beacon_filter(out: &mut PathogenFilterParams, f: &BeaconFilter) {
    if !f.id.eq_ignore_ascii_case("PathoGenFilter") {
        return;
    }
    if f.organism.is_some() {
        out.organism = f.organism.clone();
    }
    if f.amr_gene.is_some() {
        out.amr_gene = f.amr_gene.clone();
    }
    if f.serotype.is_some() {
        out.serotype = f.serotype.clone();
    }
    if f.min_qscore.is_some() {
        out.min_qscore = f.min_qscore;
    }
}

pub fn has_pathogen_params(p: &PathogenFilterParams) -> bool {
    p.organism.is_some() || p.amr_gene.is_some() || p.serotype.is_some() || p.min_qscore.is_some()
}
