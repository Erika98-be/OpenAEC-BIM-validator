use crate::{
    model::{Element, Model},
    renderer::Key,
};
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Serialize, Deserialize)]
pub struct Finding {
    pub model: usize,
    #[serde(default)]
    pub related: Option<Key>,
    pub element: Option<u32>,
    pub rule: String,
    pub message: String,
    pub severity: String,
}
impl Finding {
    pub fn key(&self) -> Option<Key> {
        self.element.map(|id| (self.model, id))
    }
}
pub fn check_models(models: &[Model]) -> Vec<Finding> {
    let mut out = vec![];
    let mut guids = HashMap::new();
    for (mi, m) in models.iter().enumerate() {
        for e in m.elements.values() {
            let mut add = |rule: &str, message: String, severity: &str| {
                out.push(Finding {
                    related: None,
                    model: mi,
                    element: Some(e.id),
                    rule: rule.into(),
                    message,
                    severity: severity.into(),
                })
            };
            if e.guid.is_empty() {
                add("GlobalId", "GlobalId ontbreekt".into(), "Fout");
            } else if let Some(previous) = guids.insert(e.guid.clone(), (mi, e.id)) {
                add(
                    "Unieke GlobalId",
                    format!(
                        "Dubbele GlobalId; ook in model {} · #{}",
                        previous.0 + 1,
                        previous.1
                    ),
                    "Fout",
                );
            }
            if e.name.trim().is_empty() {
                add("Naamgeving", "Naam ontbreekt".into(), "Waarschuwing");
            }
            if e.class == "IfcBuildingElementProxy" {
                add(
                    "Entiteit",
                    "Gebruik een specifieke IFC-entiteit in plaats van een proxy".into(),
                    "Waarschuwing",
                );
            }
        }
        for warning in &m.warnings {
            out.push(Finding {
                related: None,
                model: mi,
                element: None,
                rule: "IFC-import".into(),
                message: warning.clone(),
                severity: "Waarschuwing".into(),
            });
        }
    }
    out
}
fn children<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(|n| n.is_element())
}
fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    children(n).find(|n| n.tag_name().name() == name)
}
fn matches(n: Option<Node<'_, '_>>, value: &str) -> Result<bool, String> {
    let Some(n) = n else {
        return Ok(!value.trim().is_empty());
    };
    if let Some(simple) = child(n, "simpleValue") {
        return Ok(value == simple.text().unwrap_or(""));
    }
    let restriction = child(n, "restriction").ok_or("Onbekende IDS-waarde")?;
    let mut enums = vec![];
    for facet in children(restriction) {
        let v = facet.attribute("value").unwrap_or("");
        match facet.tag_name().name() {
            "enumeration" => enums.push(v),
            "pattern" => {
                if v.contains("\\i") || v.contains("\\c") || v.contains("-[") {
                    return Err("XML Schema-regexconstructie nog niet ondersteund".into());
                }
                if !regex::Regex::new(&format!("^(?:{v})$"))
                    .map_err(|e| e.to_string())?
                    .is_match(value)
                {
                    return Ok(false);
                }
            }
            "minInclusive" | "maxInclusive" | "minExclusive" | "maxExclusive" => {
                let actual = value.parse::<f64>().map_err(|_| "Geen numerieke waarde")?;
                let bound = v.parse::<f64>().map_err(|_| "Ongeldige IDS-grens")?;
                if !actual.is_finite() || !bound.is_finite() {
                    return Ok(false);
                }
                let ok = match facet.tag_name().name() {
                    "minInclusive" => actual >= bound,
                    "maxInclusive" => actual <= bound,
                    "minExclusive" => actual > bound,
                    _ => actual < bound,
                };
                if !ok {
                    return Ok(false);
                }
            }
            "length" | "minLength" | "maxLength" => {
                let bound = v.parse::<usize>().map_err(|_| "Ongeldige lengte")?;
                let len = value.chars().count();
                let ok = match facet.tag_name().name() {
                    "length" => len == bound,
                    "minLength" => len >= bound,
                    _ => len <= bound,
                };
                if !ok {
                    return Ok(false);
                }
            }
            _ => {
                return Err(format!(
                    "IDS-restrictie {} nog niet ondersteund",
                    facet.tag_name().name()
                ))
            }
        }
    }
    Ok(enums.is_empty() || enums.contains(&value))
}
fn facet(n: Node<'_, '_>, e: &Element) -> Result<(bool, bool), String> {
    let mut values = vec![];
    match n.tag_name().name() {
        "entity" => {
            let name = child(n, "name");
            let mut ty = Some(ifc_lite_core::IfcType::from_str(&e.class.to_uppercase()));
            let mut hit = false;
            while let Some(t) = ty {
                if matches(name, t.as_str())? {
                    hit = true;
                    break;
                }
                ty = t.parent();
            }
            if let Some(p) = child(n, "predefinedType") {
                hit &= matches(
                    Some(p),
                    e.attributes
                        .get("PredefinedType")
                        .map(String::as_str)
                        .unwrap_or(""),
                )?;
            }
            return Ok((true, hit));
        }
        "attribute" => {
            for (name, v) in &e.attributes {
                if matches(child(n, "name"), name)? && !v.trim().is_empty() {
                    values.push(v.as_str());
                }
            }
        }
        "property" => {
            for (set, props) in &e.properties {
                if matches(child(n, "propertySet"), set)? {
                    for (name, v) in props {
                        if matches(child(n, "baseName"), name)? && !v.trim().is_empty() {
                            if let Some(expected) = n.attribute("dataType") {
                                let actual = e
                                    .property_types
                                    .get(set)
                                    .and_then(|types| types.get(name))
                                    .map(String::as_str)
                                    .unwrap_or("");
                                if actual.is_empty() {
                                    return Err(format!(
                                        "Datatype van {set}.{name} niet beschikbaar"
                                    ));
                                }
                                if !actual.eq_ignore_ascii_case(expected) {
                                    return Ok((true, false));
                                }
                            }
                            values.push(v.as_str());
                        }
                    }
                }
            }
        }
        "material" => values.extend(e.materials.iter().map(String::as_str)),
        "classification" => {
            for (system, v) in &e.classifications {
                if matches(child(n, "system"), system)? {
                    values.push(v.as_str());
                }
            }
        }
        other => return Err(format!("IDS-facet {other} nog niet ondersteund")),
    }
    let present = !values.is_empty();
    let mut valid = false;
    for v in values {
        valid |= matches(child(n, "value"), v)?;
    }
    Ok((present, valid))
}
pub fn check_ids(models: &[Model], xml: &str) -> Result<Vec<Finding>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    if doc.root_element().tag_name().name() != "ids" {
        return Err("Geen IDS-document".into());
    }
    if !doc.descendants().any(|n| n.has_tag_name("specification")) {
        return Err("IDS bevat geen specificaties".into());
    }
    let mut out = vec![];
    for spec in doc
        .descendants()
        .filter(|n| n.has_tag_name("specification"))
    {
        let rule = spec.attribute("name").unwrap_or("IDS-specificatie");
        let app = child(spec, "applicability").ok_or("Applicability ontbreekt")?;
        for (mi, m) in models.iter().enumerate() {
            let mut unsupported = None;
            let mut applicable = vec![];
            if let Some(versions) = spec.attribute("ifcVersion") {
                if !versions
                    .split_whitespace()
                    .any(|v| m.schema == v || m.schema.starts_with(&format!("{v}_")))
                {
                    out.push(Finding {
                        related: None,
                        model: mi,
                        element: None,
                        rule: rule.into(),
                        message: format!("Niet van toepassing op {}", m.schema),
                        severity: "Overgeslagen".into(),
                    });
                    continue;
                }
            }
            for e in m.elements.values() {
                let mut hit = true;
                for f in children(app) {
                    match facet(f, e) {
                        Ok((_, v)) => hit &= v,
                        Err(err) => {
                            unsupported = Some(err);
                            hit = false;
                            break;
                        }
                    }
                }
                if hit {
                    applicable.push(e);
                }
            }
            let start = out.len();
            let min = app
                .attribute("minOccurs")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "Ongeldige minOccurs")?;
            let max = app
                .attribute("maxOccurs")
                .filter(|v| *v != "unbounded")
                .map(str::parse::<usize>)
                .transpose()
                .map_err(|_| "Ongeldige maxOccurs")?;
            if applicable.len() < min || max.is_some_and(|max| applicable.len() > max) {
                out.push(Finding {
                    related: None,
                    model: mi,
                    element: None,
                    rule: rule.into(),
                    message: format!(
                        "{} passende objecten; verwacht min. {min}, max. {}",
                        applicable.len(),
                        app.attribute("maxOccurs").unwrap_or("onbeperkt")
                    ),
                    severity: "Fout".into(),
                });
            }
            if let Some(req) = child(spec, "requirements") {
                for e in applicable {
                    for f in children(req) {
                        match facet(f, e) {
                            Ok((present, valid)) => {
                                let pass = match f.attribute("cardinality").unwrap_or("required") {
                                    "prohibited" => !valid,
                                    "optional" => !present || valid,
                                    _ => present && valid,
                                };
                                if !pass {
                                    out.push(Finding {
                                        related: None,
                                        model: mi,
                                        element: Some(e.id),
                                        rule: rule.into(),
                                        message: f
                                            .attribute("instructions")
                                            .unwrap_or(f.tag_name().name())
                                            .to_owned(),
                                        severity: "Fout".into(),
                                    });
                                }
                            }
                            Err(err) => unsupported = Some(err),
                        }
                    }
                }
            }
            if let Some(err) = unsupported {
                out.push(Finding {
                    related: None,
                    model: mi,
                    element: None,
                    rule: rule.into(),
                    message: err,
                    severity: "Niet ondersteund".into(),
                });
            } else if out.len() == start {
                out.push(Finding {
                    related: None,
                    model: mi,
                    element: None,
                    rule: rule.into(),
                    message: "Ondersteunde controles geslaagd".into(),
                    severity: "Geslaagd".into(),
                });
            }
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_failure_links_to_object() {
        let m = Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap();
        let xml = r#"<ids><specifications><specification name="Wall name"><applicability><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></applicability><requirements><attribute><name><simpleValue>Name</simpleValue></name><value><simpleValue>Wrong</simpleValue></value></attribute></requirements></specification></specifications></ids>"#;
        let r = check_ids(&[m], xml).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].element, Some(42));
        assert_eq!(r[0].severity, "Fout");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    fn model() -> Model {
        Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/wall-properties.ifc"
            )),
            |_| {},
        )
        .unwrap()
    }
    fn ids(requirement: &str) -> String {
        format!(
            r#"<ids><specifications><specification name="Test"><applicability><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></applicability><requirements>{requirement}</requirements></specification></specifications></ids>"#
        )
    }
    #[test]
    fn typed_boolean_property_and_material_pass() {
        let m = model();
        assert_eq!(
            m.elements[&42].property_types["Pset_WallCommon"]["LoadBearing"],
            "IFCBOOLEAN"
        );
        let xml = ids(
            r#"<property dataType="IFCBOOLEAN"><propertySet><simpleValue>Pset_WallCommon</simpleValue></propertySet><baseName><simpleValue>LoadBearing</simpleValue></baseName><value><simpleValue>TRUE</simpleValue></value></property><material><value><simpleValue>Concrete</simpleValue></value></material>"#,
        );
        let r = check_ids(&[m], &xml).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].severity, "Geslaagd");
    }
    #[test]
    fn unsupported_facet_never_passes() {
        let r = check_ids(&[model()], &ids("<partOf/>")).unwrap();
        assert!(r.iter().any(|r| r.severity == "Niet ondersteund"));
        assert!(!r.iter().any(|r| r.severity == "Geslaagd"));
    }
    #[test]
    fn wrong_datatype_fails() {
        let xml = ids(
            r#"<property dataType="IFCLABEL"><propertySet><simpleValue>Pset_WallCommon</simpleValue></propertySet><baseName><simpleValue>LoadBearing</simpleValue></baseName></property>"#,
        );
        let r = check_ids(&[model()], &xml).unwrap();
        assert_eq!(r[0].severity, "Fout");
        assert_eq!(r[0].element, Some(42));
    }
    #[test]
    fn empty_ids_is_error() {
        assert!(check_ids(&[model()], "<ids/>").is_err());
    }
}
