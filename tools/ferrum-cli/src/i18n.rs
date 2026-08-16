// SPDX-License-Identifier: BUSL-1.1
//! CLI localisation via `FERRUM_LANG` (`en`, `fr`, `de`).

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
    De,
}

pub fn current_lang() -> Lang {
    match std::env::var("FERRUM_LANG")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "fr" | "fr-fr" => Lang::Fr,
        "de" | "de-de" => Lang::De,
        _ => Lang::En,
    }
}

pub fn about(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "GA4GH Ferrum management CLI",
        Lang::Fr => "CLI de gestion GA4GH Ferrum",
        Lang::De => "GA4GH Ferrum Verwaltungs-CLI",
    }
}

pub fn health_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Show service health",
        Lang::Fr => "Afficher l'état de santé du service",
        Lang::De => "Dienstgesundheit anzeigen",
    }
}

pub fn demo_start_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Start Ferrum (Docker demo or Edge mode fallback)",
        Lang::Fr => "Démarrer Ferrum (démo Docker ou mode Edge)",
        Lang::De => "Ferrum starten (Docker-Demo oder Edge-Modus-Fallback)",
    }
}

pub fn demo_edge_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Force embedded SQLite + local storage (alias: --offline)",
        Lang::Fr => "Forcer SQLite embarqué + stockage local (alias : --offline)",
        Lang::De => "Eingebettetes SQLite + lokale Speicherung erzwingen (Alias: --offline)",
    }
}

pub fn demo_offline_help(lang: Lang) -> &'static str {
    demo_edge_help(lang)
}

pub fn demo_force_production_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Fail if PostgreSQL/MinIO are unavailable (no Edge mode fallback)",
        Lang::Fr => "Échouer si PostgreSQL/MinIO sont indisponibles (sans repli Edge)",
        Lang::De => "Fehler wenn PostgreSQL/MinIO nicht verfügbar (kein Edge-Fallback)",
    }
}

pub fn migrations_ok(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Database migrations applied successfully.",
        Lang::Fr => "Migrations de base de données appliquées avec succès.",
        Lang::De => "Datenbankmigrationen erfolgreich angewendet.",
    }
}

pub fn edge_start(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "[ferrum] Starting in Edge mode (SQLite + local storage).",
        Lang::Fr => "[ferrum] Démarrage en mode Edge (SQLite + stockage local).",
        Lang::De => "[ferrum] Start im Edge-Modus (SQLite + lokaler Speicher).",
    }
}

pub fn laptop_start(lang: Lang) -> &'static str {
    edge_start(lang)
}

pub fn edge_data_dir(lang: Lang, path: &str) -> String {
    match lang {
        Lang::En => format!("[ferrum] Data will be stored at {path}/"),
        Lang::Fr => format!("[ferrum] Les données seront stockées dans {path}/"),
        Lang::De => format!("[ferrum] Daten werden gespeichert unter {path}/"),
    }
}

pub fn laptop_data_dir(lang: Lang, path: &str) -> String {
    edge_data_dir(lang, path)
}

pub fn production_config_hint(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "[ferrum] To use production backends, set FERRUM_CONFIG=/path/to/config.toml",
        Lang::Fr => "[ferrum] Pour les backends de production, définissez FERRUM_CONFIG=/chemin/vers/config.toml",
        Lang::De => "[ferrum] Für Produktions-Backends setzen Sie FERRUM_CONFIG=/pfad/zu/config.toml",
    }
}

pub fn production_timeout(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "Production services (PostgreSQL/MinIO) did not become ready within 30 seconds. Start dependencies or use --offline."
        }
        Lang::Fr => {
            "Les services de production (PostgreSQL/MinIO) ne sont pas prêts après 30 secondes. Démarrez les dépendances ou utilisez --offline."
        }
        Lang::De => {
            "Produktionsdienste (PostgreSQL/MinIO) waren nach 30 Sekunden nicht bereit. Starten Sie Abhängigkeiten oder nutzen Sie --offline."
        }
    }
}

pub fn production_fallback(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "[ferrum] Production services not ready within 30s; falling back to Edge mode.",
        Lang::Fr => {
            "[ferrum] Services de production non prêts sous 30 s ; repli vers le mode Edge."
        }
        Lang::De => "[ferrum] Produktionsdienste nach 30 s nicht bereit; Fallback auf Edge-Modus.",
    }
}

pub fn docker_not_implemented(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "Docker demo not implemented in ferrum-cli; use ferrum-gateway demo start or --edge"
        }
        Lang::Fr => {
            "Démo Docker non implémentée dans ferrum-cli ; utilisez ferrum-gateway demo start ou --edge"
        }
        Lang::De => {
            "Docker-Demo in ferrum-cli nicht implementiert; nutzen Sie ferrum-gateway demo start oder --edge"
        }
    }
}

pub fn field_edge_tagline(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Ferrum Field Edge — offline GA4GH on Pi and lab laptops",
        Lang::Fr => "Ferrum Field Edge — GA4GH hors ligne sur Pi et portables",
        Lang::De => "Ferrum Field Edge — Offline-GA4GH auf Pi und Feldlaptops",
    }
}

pub fn field_sync_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Field sync queue (Edge to hub upload)",
        Lang::Fr => "File d'attente de sync terrain (Edge vers hub)",
        Lang::De => "Feldsync-Warteschlange (Edge zum Hub)",
    }
}

pub fn field_backup_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "SQLite backup and integrity verify (Edge)",
        Lang::Fr => "Sauvegarde SQLite et vérification d'intégrité (Edge)",
        Lang::De => "SQLite-Backup und Integritätsprüfung (Edge)",
    }
}

pub fn field_pipeline_help(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Field analysis pipeline (QC, Beacon, WES forward)",
        Lang::Fr => "Pipeline d'analyse terrain (QC, Beacon, envoi WES)",
        Lang::De => "Feldpipeline (QC, Beacon, WES-Weiterleitung)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_french_cli_output() {
        assert!(about(Lang::Fr).contains("CLI"));
        assert!(health_help(Lang::Fr).contains("santé"));
        assert!(demo_start_help(Lang::Fr).contains("Démarrer"));
    }

    #[test]
    fn field_edge_strings() {
        assert!(field_edge_tagline(Lang::En).contains("Field Edge"));
        assert!(field_sync_help(Lang::Fr).contains("sync"));
        assert!(field_backup_help(Lang::De).contains("SQLite"));
        assert!(field_pipeline_help(Lang::En).contains("Beacon"));
        assert!(edge_start(Lang::De).contains("Edge-Modus"));
    }
}
