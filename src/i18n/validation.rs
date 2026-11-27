use super::Language;
use super::catalog::CATALOG;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct LanguageCoverage {
    pub code: String,
    pub total_keys: usize,
    pub translated_keys: usize,
    pub empty_keys: Vec<String>,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub total_keys: usize,
    pub duplicate_keys: Vec<String>,
    pub placeholder_mismatches: Vec<String>,
    pub languages: Vec<LanguageCoverage>,
}

fn count_placeholders(s: &str) -> usize {
    s.matches("{}").count()
}

pub fn validate_catalog() -> Result<CoverageReport, Vec<String>> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    let mut placeholder_mismatches = Vec::new();
    let mut errors = Vec::new();

    for entry in CATALOG {
        if !seen.insert(entry.key) {
            duplicates.push(entry.key.to_string());
            errors.push(format!("Duplicate localization key: {}", entry.key));
        }
        let en_ph = count_placeholders(entry.en);
        for lang in [
            Language::Fa,
            Language::Ar,
            Language::Tr,
            Language::De,
            Language::Fr,
            Language::Es,
            Language::Ru,
        ] {
            let val = entry.get(lang);
            if val.trim().is_empty() {
                errors.push(format!("Empty translation for {} in {:?}", entry.key, lang));
            }
            if count_placeholders(val) != en_ph {
                placeholder_mismatches.push(format!("{}:{:?}", entry.key, lang));
                errors.push(format!(
                    "Placeholder count mismatch for {} in {:?}",
                    entry.key, lang
                ));
            }
        }
    }

    let langs = [
        ("en", Language::En),
        ("fa", Language::Fa),
        ("ar", Language::Ar),
        ("tr", Language::Tr),
        ("de", Language::De),
        ("fr", Language::Fr),
        ("es", Language::Es),
        ("ru", Language::Ru),
    ];

    let mut lang_reports = Vec::new();
    for (code, lang) in langs {
        let mut empty = Vec::new();
        let mut translated = 0usize;
        for entry in CATALOG {
            let v = entry.get(lang).trim();
            if v.is_empty() {
                empty.push(entry.key.to_string());
            } else {
                translated += 1;
            }
        }
        let pct = if CATALOG.is_empty() {
            100.0
        } else {
            (translated as f64 / CATALOG.len() as f64) * 100.0
        };
        lang_reports.push(LanguageCoverage {
            code: code.to_string(),
            total_keys: CATALOG.len(),
            translated_keys: translated,
            empty_keys: empty,
            coverage_percent: pct,
        });
    }

    let report = CoverageReport {
        total_keys: CATALOG.len(),
        duplicate_keys: duplicates,
        placeholder_mismatches,
        languages: lang_reports,
    };

    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}
