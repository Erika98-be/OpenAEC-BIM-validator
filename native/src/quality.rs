//! Explicit, auditable checks. The score describes these checks, not compliance.
use crate::{
    model::Model,
    validation::{self, Finding},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub material: bool,
    pub classification: bool,
    pub nlsfb: bool,
    pub common_properties: bool,
}
pub struct Report {
    pub findings: Vec<Finding>,
    pub passed: usize,
    pub checked: usize,
}
impl Report {
    pub fn score(&self) -> Option<f64> {
        (self.checked > 0).then(|| 100. * self.passed as f64 / self.checked as f64)
    }
}
pub fn run(models: &[Model], options: &Options) -> Report {
    let mut findings = validation::check_models(models);
    let mut checked = 0;
    let mut passed = 0;
    let mut counts = std::collections::HashMap::new();
    for e in models.iter().flat_map(|m| m.elements.values()) {
        *counts.entry(e.guid.as_str()).or_insert(0_usize) += 1;
    }
    for (mi, m) in models.iter().enumerate() {
        for mesh in &m.meshes {
            let Some(e) = m.elements.get(&mesh.id) else {
                continue;
            };
            // The baseline findings are already present, but form part of the score.
            checked += 3;
            passed += usize::from(!e.name.trim().is_empty())
                + usize::from(!e.guid.is_empty() && counts.get(e.guid.as_str()) == Some(&1))
                + usize::from(e.class != "IfcBuildingElementProxy");
            let mut check = |rule: &str, ok: bool| {
                checked += 1;
                if ok {
                    passed += 1;
                } else {
                    findings.push(Finding {
                        model: mi,
                        element: Some(e.id),
                        related: None,
                        rule: rule.into(),
                        message: format!("{}: {}", e.name, rule),
                        severity: "Waarschuwing".into(),
                    });
                }
            };
            if options.material {
                check("Materiaal ontbreekt", !e.materials.is_empty());
            }
            if options.classification {
                check("Classificatie ontbreekt", !e.classifications.is_empty());
            }
            if options.nlsfb {
                check(
                    "NL-SfB-classificatie ontbreekt",
                    e.classifications.iter().any(|(system, code)| {
                        system
                            .to_lowercase()
                            .chars()
                            .filter(char::is_ascii_alphanumeric)
                            .collect::<String>()
                            .contains("nlsfb")
                            && !code.trim().is_empty()
                    }),
                );
            }
            if options.common_properties {
                for key in ["IsExternal", "LoadBearing", "FireRating"] {
                    let value = e.properties.values().find_map(|p| {
                        p.iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(key))
                            .map(|(_, v)| v)
                    });
                    let valid = value.is_some_and(|v| {
                        if key == "FireRating" {
                            !v.trim().is_empty()
                        } else {
                            matches!(
                                v.to_ascii_uppercase().as_str(),
                                "TRUE" | "FALSE" | "T" | "F"
                            )
                        }
                    });
                    check(
                        &format!("Common property {key} ontbreekt of is ongeldig"),
                        valid,
                    );
                }
            }
        }
    }
    Report {
        findings,
        passed,
        checked,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_project_has_no_misleading_perfect_score() {
        assert_eq!(run(&[], &Options::default()).score(), None);
    }
    #[test]
    fn extra_required_metadata_changes_denominator_and_findings() {
        let m = Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap();
        let base = run(std::slice::from_ref(&m), &Options::default());
        let extra = run(
            &[m],
            &Options {
                nlsfb: true,
                ..Default::default()
            },
        );
        assert!(extra.checked > base.checked);
        assert!(extra.findings.iter().any(|f| f.rule.contains("NL-SfB")));
        assert!(extra.score().unwrap() < base.score().unwrap());
    }
}
