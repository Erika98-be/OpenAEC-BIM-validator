//! Native property schedules and GUID-based revision comparison.
use crate::{
    model::{Element, Model},
    renderer::Key,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Query {
    pub text: String,
    pub class: String,
    pub property: String,
    pub value: String,
    pub operator: usize,
}

pub fn property(element: &Element, name: &str) -> String {
    match name {
        "Name" => element.name.clone(),
        "GlobalId" => element.guid.clone(),
        "Type" => element.class.clone(),
        "Material" => element.materials.join("; "),
        _ => {
            if let Some((set, key)) = name.split_once('.') {
                element
                    .properties
                    .get(set)
                    .and_then(|p| p.get(key))
                    .cloned()
                    .unwrap_or_default()
            } else {
                element.attributes.get(name).cloned().unwrap_or_default()
            }
        }
    }
}
impl Query {
    pub fn matches(&self, element: &Element) -> bool {
        let text = self.text.to_lowercase();
        if !text.is_empty()
            && !format!("{} {} {}", element.name, element.guid, element.class)
                .to_lowercase()
                .contains(&text)
        {
            return false;
        }
        if !self.class.is_empty() && self.class != element.class {
            return false;
        }
        if self.property.is_empty() {
            return true;
        }
        let actual = property(element, &self.property);
        match self.operator {
            1 => actual.eq_ignore_ascii_case(&self.value),
            2 => actual.is_empty(),
            3 => !actual.is_empty(),
            4 | 5 => actual
                .parse::<f64>()
                .ok()
                .zip(self.value.parse::<f64>().ok())
                .is_some_and(|(a, b)| {
                    a.is_finite() && b.is_finite() && if self.operator == 4 { a > b } else { a < b }
                }),
            _ => actual.to_lowercase().contains(&self.value.to_lowercase()),
        }
    }
    pub fn rows(&self, models: &[Model]) -> Vec<Key> {
        models
            .iter()
            .enumerate()
            .flat_map(|(mi, m)| {
                m.elements
                    .values()
                    .filter(|e| self.matches(e))
                    .map(move |e| (mi, e.id))
            })
            .collect()
    }
}

pub fn columns(models: &[Model]) -> Vec<String> {
    let mut names = BTreeSet::from(["Material".into()]);
    for m in models {
        for e in m.elements.values() {
            for (set, props) in &e.properties {
                for key in props.keys() {
                    names.insert(format!("{set}.{key}"));
                }
            }
        }
    }
    names.into_iter().collect()
}
fn cell(value: &str) -> String {
    // Protect spreadsheets against formula injection, including whitespace prefixes.
    let safe = if value.trim_start().starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}
pub fn csv(models: &[Model], rows: &[Key], column: &str) -> String {
    let mut out = format!("Model,STEP,GlobalId,Type,Name,{}\r\n", cell(column));
    for &(mi, id) in rows {
        if let Some((m, e)) = models
            .get(mi)
            .and_then(|m| m.elements.get(&id).map(|e| (m, e)))
        {
            out.push_str(
                &[
                    m.name.clone(),
                    id.to_string(),
                    e.guid.clone(),
                    e.class.clone(),
                    e.name.clone(),
                    property(e, column),
                ]
                .iter()
                .map(|s| cell(s))
                .collect::<Vec<_>>()
                .join(","),
            );
            out.push_str("\r\n");
        }
    }
    out
}

#[derive(Clone, Debug, Serialize)]
pub struct Difference {
    pub guid: String,
    pub name: String,
    pub status: String,
    pub changes: Vec<String>,
    pub before: Option<Key>,
    pub after: Option<Key>,
}

/// Duplicate or missing GUIDs cannot safely establish revision identity.
pub fn compare(models: &[Model], before: usize, after: usize) -> Result<Vec<Difference>, String> {
    if before == after {
        return Err("Kies twee verschillende modelversies".into());
    }
    let a = models.get(before).ok_or("Oud model ontbreekt")?;
    let b = models.get(after).ok_or("Nieuw model ontbreekt")?;
    fn index(model: &Model) -> Result<BTreeMap<String, &Element>, String> {
        let mut result = BTreeMap::new();
        for e in model.elements.values() {
            if e.guid.is_empty() {
                continue;
            }
            if result.insert(e.guid.clone(), e).is_some() {
                return Err(format!("Dubbele GlobalId in {}: {}", model.name, e.guid));
            }
        }
        Ok(result)
    }
    let ai = index(a)?;
    let bi = index(b)?;
    let guids: BTreeSet<_> = ai.keys().chain(bi.keys()).collect();
    let mut result = vec![];
    for guid in guids {
        let old = ai.get(guid).copied();
        let new = bi.get(guid).copied();
        let mut changes = vec![];
        let status = match (old, new) {
            (None, Some(_)) => "Toegevoegd",
            (Some(_), None) => "Verwijderd",
            (Some(x), Some(y)) => {
                if x.name != y.name {
                    changes.push("Naam".into());
                }
                if x.class != y.class {
                    changes.push("IFC-type".into());
                }
                // STEP reference numbers change on export; compare semantic attributes only.
                let attrs = |e: &Element| {
                    e.attributes
                        .iter()
                        .filter(|(k, v)| {
                            !matches!(
                                k.as_str(),
                                "GlobalId"
                                    | "Name"
                                    | "OwnerHistory"
                                    | "ObjectPlacement"
                                    | "Representation"
                            ) && !v.contains('#')
                        })
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<BTreeMap<_, _>>()
                };
                if attrs(x) != attrs(y) {
                    changes.push("Attributen".into());
                }
                if x.properties != y.properties || x.property_types != y.property_types {
                    changes.push("Eigenschappen".into());
                }
                if x.materials != y.materials {
                    changes.push("Materialen".into());
                }
                if x.classifications != y.classifications {
                    changes.push("Classificatie".into());
                }
                let points = |m: &Model, id: u32| {
                    let mut triangles = Vec::new();
                    for mesh in m.meshes.iter().filter(|m| m.id == id) {
                        for tri in mesh.indices.as_chunks::<3>().0 {
                            let mut p = tri
                                .iter()
                                .map(|&i| {
                                    let v = mesh.vertices[i as usize];
                                    let world = mesh.origin
                                        + glam::DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
                                    world.to_array().map(|x| (x * 1000.).round() as i64)
                                })
                                .collect::<Vec<_>>();
                            p.sort();
                            triangles.push(p);
                        }
                    }
                    triangles.sort();
                    triangles
                };
                if points(a, x.id) != points(b, y.id) {
                    changes.push("Mesh / positie (1 mm raster)".into());
                }
                if changes.is_empty() {
                    continue;
                }
                "Gewijzigd"
            }
            _ => continue,
        };
        result.push(Difference {
            guid: guid.clone(),
            name: new.or(old).unwrap().name.clone(),
            status: status.into(),
            changes,
            before: old.map(|e| (before, e.id)),
            after: new.map(|e| (after, e.id)),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Model {
        Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap()
    }
    #[test]
    fn csv_escapes_formulas_quotes_and_newlines() {
        assert_eq!(cell(" =1+1"), "\"' =1+1\"");
        assert_eq!(cell("a\"b\nc"), "\"a\"\"b\nc\"");
    }
    #[test]
    fn revision_detects_property_changes_and_removal() {
        let a = model();
        let mut b = model();
        let id = *b
            .elements
            .iter()
            .find(|(_, e)| !e.guid.is_empty())
            .unwrap()
            .0;
        b.elements.get_mut(&id).unwrap().name = "Changed name".into();
        let mut models = vec![a, b];
        let diff = compare(&models, 0, 1).unwrap();
        assert_eq!(diff.len(), 1);
        assert!(diff[0].changes.contains(&"Naam".into()));
        models[1].elements.remove(&id);
        assert_eq!(compare(&models, 0, 1).unwrap()[0].status, "Verwijderd");
    }
    #[test]
    fn numeric_filter_is_numeric() {
        let m = model();
        let mut e = m.elements.values().next().unwrap().clone();
        e.attributes.insert("Count".into(), "10".into());
        let q = Query {
            property: "Count".into(),
            value: "2".into(),
            operator: 4,
            ..Default::default()
        };
        assert!(q.matches(&e));
        e.attributes.insert("Count".into(), "NaN".into());
        assert!(!q.matches(&e));
    }
}
