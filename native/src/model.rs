use glam::{DVec3, Vec3};
use ifc_lite_core::{
    attribute_names_for_schema, build_entity_index, declared_schema_bounded, AttributeValue,
    DecodedEntity, EntityDecoder, EntityScanner, IfcType,
};
use ifc_lite_geometry::GeometryRouter;
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Element {
    pub id: u32,
    pub class: String,
    pub name: String,
    pub guid: String,
    pub parent: Option<u32>,
    pub attributes: BTreeMap<String, String>,
    pub properties: BTreeMap<String, BTreeMap<String, String>>,
    pub property_types: BTreeMap<String, BTreeMap<String, String>>,
    pub materials: Vec<String>,
    pub classifications: Vec<(String, String)>,
}
#[derive(Clone)]
pub struct Mesh {
    pub id: u32,
    /// Small IFC-local positions; the world origin must stay in double precision.
    pub vertices: Vec<[f32; 6]>,
    pub origin: DVec3,
    pub indices: Vec<u32>,
    pub min: DVec3,
    pub max: DVec3,
    pub color: [f32; 3],
}
#[derive(Clone)]
pub struct Model {
    pub name: String,
    pub path: PathBuf,
    pub schema: String,
    pub elements: BTreeMap<u32, Element>,
    pub meshes: Vec<Mesh>,
    pub warnings: Vec<String>,
}
fn value(v: &AttributeValue) -> String {
    match v {
        AttributeValue::String(v) | AttributeValue::Enum(v) => v.clone(),
        AttributeValue::Integer(v) => v.to_string(),
        AttributeValue::Float(v) => v.to_string(),
        AttributeValue::EntityRef(v) => format!("#{v}"),
        AttributeValue::List(v) => v.iter().map(value).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}
fn property_set(
    e: &DecodedEntity,
    all: &HashMap<u32, DecodedEntity>,
    types: &HashMap<u32, String>,
) -> (String, BTreeMap<String, String>, BTreeMap<String, String>) {
    let index = if e.ifc_type == IfcType::IfcElementQuantity {
        5
    } else {
        4
    };
    let props = e
        .get_refs(index)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| {
            let p = all.get(&id)?;
            let vi = if p.ifc_type.name().starts_with("IfcQuantity") {
                3
            } else {
                2
            };
            let raw = p.get(vi);
            let typed = if p.ifc_type == IfcType::IfcPropertySingleValue {
                raw.and_then(AttributeValue::as_list).and_then(|v| v.get(1))
            } else {
                None
            };
            let mut text = typed.or(raw).map(value).unwrap_or_default();
            if types
                .get(&id)
                .is_some_and(|t| t == "IFCBOOLEAN" || t == "IFCLOGICAL")
            {
                text = match text.as_str() {
                    "T" => "TRUE".into(),
                    "F" => "FALSE".into(),
                    "U" => "UNKNOWN".into(),
                    _ => text,
                };
            }
            Some((p.get_string(0)?.to_owned(), text))
        })
        .collect();
    let property_types = e
        .get_refs(index)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| {
            let p = all.get(&id)?;
            Some((
                p.get_string(0)?.to_owned(),
                types.get(&id).cloned().unwrap_or_default(),
            ))
        })
        .collect();
    (
        e.get_string(2).unwrap_or("Eigenschappen").to_owned(),
        props,
        property_types,
    )
}
impl Model {
    pub fn load(path: &Path, progress: impl Fn(String)) -> Result<Self, String> {
        progress("IFC lezen…".into());
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let schema = declared_schema_bounded(&data)
            .ok_or("Geen geldig IFC STEP-bestand: FILE_SCHEMA ontbreekt")?;
        if !["IFC2X3", "IFC4", "IFC4X3"]
            .iter()
            .any(|s| schema.starts_with(s))
        {
            return Err(format!("Niet-ondersteund IFC-schema: {schema}"));
        }
        let mut decoder = EntityDecoder::with_index(&data, build_entity_index(&data));
        let mut scanner = EntityScanner::new(&data);
        let mut all = HashMap::new();
        let mut warnings = Vec::new();
        let mut property_types = HashMap::new();
        while let Some((id, _, start, end)) = scanner.next_entity() {
            match decoder.decode_by_id(id) {
                Ok(e) => {
                    if e.ifc_type == IfcType::IfcPropertySingleValue {
                        if let Ok((_, _, tokens)) = ifc_lite_core::parse_entity(&data[start..end]) {
                            if let Some(ifc_lite_core::Token::TypedValue(name, _)) = tokens.get(2) {
                                property_types
                                    .insert(id, String::from_utf8_lossy(name).to_uppercase());
                            }
                        }
                    } else if e.ifc_type.name().starts_with("IfcQuantity") {
                        let ty = match e.ifc_type.name() {
                            "IfcQuantityLength" => "IFCLENGTHMEASURE",
                            "IfcQuantityArea" => "IFCAREAMEASURE",
                            "IfcQuantityVolume" => "IFCVOLUMEMEASURE",
                            "IfcQuantityCount" => "IFCCOUNTMEASURE",
                            "IfcQuantityWeight" => "IFCMASSMEASURE",
                            "IfcQuantityTime" => "IFCTIMEMEASURE",
                            _ => "",
                        };
                        property_types.insert(id, ty.into());
                    }
                    all.insert(id, e);
                }
                Err(e) => warnings.push(format!("#{id}: {e}")),
            }
        }
        if all.is_empty() {
            return Err("Het IFC-bestand bevat geen leesbare entiteiten".into());
        }
        let mut elements = BTreeMap::new();
        for e in all
            .values()
            .filter(|e| e.ifc_type.is_subtype_of(IfcType::IfcObjectDefinition))
        {
            let names = attribute_names_for_schema(&schema, e.ifc_type.as_str())
                .unwrap_or_else(|| e.ifc_type.attribute_names());
            elements.insert(
                e.id,
                Element {
                    id: e.id,
                    class: e.ifc_type.name().into(),
                    name: e.get_string(2).unwrap_or("").into(),
                    guid: e.get_string(0).unwrap_or("").into(),
                    parent: None,
                    attributes: names
                        .iter()
                        .zip(e.attributes.iter())
                        .map(|(n, v)| (n.to_string(), value(v)))
                        .collect(),
                    properties: BTreeMap::new(),
                    property_types: BTreeMap::new(),
                    materials: vec![],
                    classifications: vec![],
                },
            );
        }
        let mut voids = rustc_hash::FxHashMap::default();
        // Inherited type properties first, then occurrence-level overrides.
        for rel in all
            .values()
            .filter(|e| e.ifc_type == IfcType::IfcRelDefinesByType)
        {
            if let Some(ty) = rel.get_ref(5).and_then(|id| all.get(&id)) {
                for id in rel.get_refs(4).unwrap_or_default() {
                    if let Some(element) = elements.get_mut(&id) {
                        for ps in ty
                            .get_refs(5)
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|id| all.get(id))
                        {
                            let (name, props, types) = property_set(ps, &all, &property_types);
                            element.property_types.insert(name.clone(), types);
                            element.properties.insert(name, props);
                        }
                    }
                }
            }
        }
        for rel in all.values() {
            match rel.ifc_type {
                IfcType::IfcRelAggregates
                | IfcType::IfcRelNests
                | IfcType::IfcRelContainedInSpatialStructure => {
                    let (parent, children) =
                        if rel.ifc_type == IfcType::IfcRelContainedInSpatialStructure {
                            (rel.get_ref(5), rel.get_refs(4))
                        } else {
                            (rel.get_ref(4), rel.get_refs(5))
                        };
                    for id in children.unwrap_or_default() {
                        if let Some(e) = elements.get_mut(&id) {
                            e.parent = parent;
                        }
                    }
                }
                IfcType::IfcRelVoidsElement => {
                    if let (Some(host), Some(opening)) = (rel.get_ref(4), rel.get_ref(5)) {
                        voids.entry(host).or_insert_with(Vec::new).push(opening);
                    }
                }
                IfcType::IfcRelDefinesByProperties => {
                    if let Some(ps) = rel.get_ref(5).and_then(|id| all.get(&id)) {
                        let (name, props, types) = property_set(ps, &all, &property_types);
                        for id in rel.get_refs(4).unwrap_or_default() {
                            if let Some(e) = elements.get_mut(&id) {
                                e.property_types
                                    .entry(name.clone())
                                    .or_default()
                                    .extend(types.clone());
                                e.properties
                                    .entry(name.clone())
                                    .or_default()
                                    .extend(props.clone());
                            }
                        }
                    }
                }
                IfcType::IfcRelAssociatesMaterial => {
                    if let Some(id) = rel.get_ref(5) {
                        let mut names = vec![];
                        collect_material(
                            id,
                            &all,
                            &mut names,
                            &mut std::collections::HashSet::new(),
                        );
                        for id in rel.get_refs(4).unwrap_or_default() {
                            if let Some(e) = elements.get_mut(&id) {
                                e.materials.extend(names.clone());
                            }
                        }
                    }
                }
                IfcType::IfcRelAssociatesClassification => {
                    if let Some(c) = rel.get_ref(5).and_then(|id| all.get(&id)) {
                        let code = c.get_string(1).unwrap_or("").to_owned();
                        let system = if c.ifc_type == IfcType::IfcClassification {
                            c.get_string(3)
                        } else {
                            c.get_ref(3)
                                .and_then(|id| all.get(&id))
                                .and_then(|e| e.get_string(3))
                        }
                        .unwrap_or("")
                        .to_owned();
                        for id in rel.get_refs(4).unwrap_or_default() {
                            if let Some(e) = elements.get_mut(&id) {
                                e.classifications.push((system.clone(), code.clone()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let mut router = GeometryRouter::with_units(&data, &mut decoder);
        let rtc = router
            .detect_rtc_anchor_for_file(&data, &mut decoder)
            .unwrap_or((0., 0., 0.));
        router.set_rtc_offset(rtc);
        let reference = DVec3::new(rtc.0, rtc.1, rtc.2);
        let products: Vec<_> = all
            .values()
            .filter(|e| {
                e.ifc_type.is_subtype_of(IfcType::IfcProduct)
                    && e.get_ref(6).is_some()
                    && e.ifc_type != IfcType::IfcOpeningElement
            })
            .collect();
        let mut meshes = Vec::new();
        for (i, e) in products.iter().enumerate() {
            if i % 10 == 0 {
                progress(format!("Geometrie {}/{}", i + 1, products.len()));
            }
            match router.process_element_with_voids(e, &mut decoder, &voids) {
                Ok(m) if !m.indices.is_empty() => {
                    let origin = DVec3::from_array(m.origin) + reference;
                    let mut min = DVec3::splat(f64::INFINITY);
                    let mut max = DVec3::splat(f64::NEG_INFINITY);
                    let vertices: Vec<_> = m
                        .positions
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let p = Vec3::from_slice(p);
                            let world = p.as_dvec3() + origin;
                            min = min.min(world);
                            max = max.max(world);
                            let n = m.normals.get(i * 3..i * 3 + 3).unwrap_or(&[0., 0., 1.]);
                            [p.x, p.y, p.z, n[0], n[1], n[2]]
                        })
                        .collect();
                    if !min.is_finite()
                        || !max.is_finite()
                        || m.indices.iter().any(|i| *i as usize >= vertices.len())
                    {
                        warnings.push(format!("#{}: ongeldige geometrie", e.id));
                        continue;
                    }
                    meshes.push(Mesh {
                        id: e.id,
                        origin,
                        vertices,
                        indices: m.indices,
                        min,
                        max,
                        color: class_color(e.ifc_type.name()),
                    });
                }
                Ok(_) => warnings.push(format!(
                    "#{} {}: geen driehoeken gegenereerd",
                    e.id,
                    e.ifc_type.name()
                )),
                Err(err) => warnings.push(format!("#{} {}: {err}", e.id, e.ifc_type.name())),
            }
        }
        for (kind, count) in router.take_unsupported_items() {
            warnings.push(format!(
                "IfcLite: {count} niet-ondersteunde geometrie-items van type {kind} overgeslagen"
            ));
        }
        warnings.sort();
        meshes.sort_by_key(|m| m.id);
        Ok(Self {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path: path.into(),
            schema,
            elements,
            meshes,
            warnings,
        })
    }
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}
fn collect_material(
    id: u32,
    all: &HashMap<u32, DecodedEntity>,
    out: &mut Vec<String>,
    visited: &mut std::collections::HashSet<u32>,
) {
    if !visited.insert(id) || visited.len() > 10000 {
        return;
    }
    if let Some(e) = all.get(&id) {
        if e.ifc_type == IfcType::IfcMaterial {
            if let Some(name) = e.get_string(0) {
                out.push(name.into());
            }
            return;
        }
        for v in e.attributes.iter() {
            match v {
                AttributeValue::EntityRef(id) => collect_material(*id, all, out, visited),
                AttributeValue::List(vs) => {
                    for v in vs {
                        if let Some(id) = v.as_entity_ref() {
                            collect_material(id, all, out, visited);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
pub fn class_color(class: &str) -> [f32; 3] {
    if class.contains("Wall") {
        [0.78, 0.76, 0.70]
    } else if class.contains("Slab") {
        [0.57, 0.65, 0.70]
    } else if class.contains("Window") {
        [0.36, 0.70, 0.83]
    } else if class.contains("Door") {
        [0.70, 0.46, 0.25]
    } else if class.contains("Beam") || class.contains("Column") {
        [0.69, 0.63, 0.48]
    } else if class.contains("Space") {
        [0.48, 0.72, 0.59]
    } else if class.contains("Roof") {
        [0.69, 0.40, 0.31]
    } else {
        [0.67, 0.69, 0.73]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_ifc_has_hierarchy_geometry_and_metadata() {
        let m = Model::load(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap();
        assert_eq!(m.schema, "IFC4");
        assert_eq!(m.elements[&42].parent, Some(23));
        assert_eq!(m.elements[&42].name, "W-001");
        assert_eq!(m.meshes.len(), 1);
        assert!(m.triangle_count() >= 12);
        assert!(m.meshes[0].max.z > 2999.); // Fixture declares metres, not millimetres.
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn georeferenced_wall_keeps_eight_millimetre_thickness() {
        let m = Model::load(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/georeferenced-thin-wall.ifc"
            )),
            |_| {},
        )
        .unwrap();
        let mesh = &m.meshes[0];
        assert!(
            (mesh.min.x - 75032.45).abs() < 0.001,
            "origin {:?}, bounds {:?} {:?}",
            mesh.origin,
            mesh.min,
            mesh.max
        );
        assert!((mesh.min.y - 454268.5).abs() < 0.001);
        assert!(((mesh.max - mesh.min).y - 0.008).abs() < 0.00001);
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for v in &mesh.vertices {
            min = min.min(v[1]);
            max = max.max(v[1]);
        }
        assert!(
            (max - min - 0.008).abs() < 0.00001,
            "local geometry was rounded: {}",
            max - min
        );
    }
    #[test]
    fn millimetre_geometry_is_converted_to_metres() {
        let m = Model::load(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/millimetres.ifc"
            )),
            |_| {},
        )
        .unwrap();
        let size = m.meshes[0].max - m.meshes[0].min;
        assert!((size.x - 5.).abs() < 0.001);
        assert!((size.y - 0.2).abs() < 0.001);
        assert!((size.z - 3.).abs() < 0.001);
    }
}

/// Coarse browsing aid derived from IFC type, not an authored discipline assignment.
pub fn discipline(class: &str) -> &'static str {
    if [
        "Flow",
        "Distribution",
        "Duct",
        "Pipe",
        "Cable",
        "AirTerminal",
        "Sanitary",
        "LightFixture",
        "Pump",
        "Fan",
        "Boiler",
    ]
    .iter()
    .any(|s| class.contains(s))
    {
        "Installaties"
    } else if [
        "Beam",
        "Column",
        "Member",
        "Plate",
        "Footing",
        "Pile",
        "Reinforcing",
    ]
    .iter()
    .any(|s| class.contains(s))
    {
        "Constructie"
    } else if [
        "Wall", "Slab", "Roof", "Door", "Window", "Stair", "Railing", "Covering", "Space",
    ]
    .iter()
    .any(|s| class.contains(s))
    {
        "Bouwkundig"
    } else {
        "Overig"
    }
}
