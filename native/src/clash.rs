use crate::{model::Mesh, renderer::Key, validation::Finding};
use ifc_lite_clash::{ClashSession, ClashStatus, DistanceKind};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub name: String,
    pub enabled: bool,
    pub two_sided: bool,
    pub model_a: Option<usize>,
    pub model_b: Option<usize>,
    pub class_a: String,
    pub class_b: String,
    pub clearance: bool,
    pub tolerance: f64,
    pub distance: f64,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            name: "Nieuwe clashregel".into(),
            enabled: true,
            two_sided: false,
            model_a: None,
            model_b: None,
            class_a: String::new(),
            class_b: String::new(),
            clearance: false,
            tolerance: 0.001,
            distance: 0.05,
        }
    }
}
impl Rule {
    pub fn valid(&self) -> bool {
        self.tolerance.is_finite()
            && self.tolerance >= 0.
            && self.distance.is_finite()
            && self.distance >= 0.
    }
}
/// Visible mesh snapshot; IFC model ids remain distinct when STEP ids overlap.
#[cfg(test)]
pub fn check(meshes: Vec<(Key, Mesh)>, tolerance: f64) -> Vec<Finding> {
    let group = (0..meshes.len() as u32).collect::<Vec<_>>();
    check_rule(
        meshes,
        &group,
        None,
        &Rule {
            tolerance,
            ..Rule::default()
        },
    )
}
pub fn check_rule(
    meshes: Vec<(Key, Mesh)>,
    group_a: &[u32],
    group_b: Option<&[u32]>,
    rule: &Rule,
) -> Vec<Finding> {
    if !rule.valid() {
        return vec![];
    }
    let reference = meshes
        .first()
        .map(|(_, m)| m.origin)
        .unwrap_or(glam::DVec3::ZERO);
    let mut positions = vec![];
    let mut pos_ranges = vec![];
    let mut indices = vec![];
    let mut idx_ranges = vec![];
    let mut bounds = vec![];
    for (_, m) in &meshes {
        pos_ranges.extend([positions.len() as u32, (m.vertices.len() * 3) as u32]);
        for v in &m.vertices {
            let p = glam::DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64) + m.origin - reference;
            positions.extend_from_slice(&p.as_vec3().to_array());
        }
        idx_ranges.extend([indices.len() as u32, m.indices.len() as u32]);
        indices.extend_from_slice(&m.indices);
        bounds.extend_from_slice(&(m.min - reference).as_vec3().to_array());
        bounds.extend_from_slice(&(m.max - reference).as_vec3().to_array());
    }
    let mut session = ClashSession::new();
    session.ingest(&positions, &pos_ranges, &indices, &idx_ranges, &bounds);
    session
        .run_rule(
            group_a,
            group_b,
            u8::from(rule.clearance),
            rule.tolerance,
            rule.distance,
            false,
        )
        .records
        .into_iter()
        .filter(|r| matches!(r.status, ClashStatus::Hard | ClashStatus::Clearance))
        .map(|r| {
            let a = meshes[r.a as usize].0;
            let b = meshes[r.b as usize].0;
            Finding {
                model: a.0,
                element: Some(a.1),
                related: Some(b),
                rule: format!(
                    "{} · {}",
                    rule.name,
                    if r.status == ClashStatus::Clearance {
                        "Vrije ruimte"
                    } else {
                        "Harde clash"
                    }
                ),
                message: format!(
                    "M{} #{} ↔ M{} #{} · {:.1} mm ({})",
                    a.0 + 1,
                    a.1,
                    b.0 + 1,
                    b.1,
                    r.distance * 1000.,
                    if r.distance_kind == DistanceKind::Mesh {
                        "meshmeting"
                    } else {
                        "schatting op begrenzing"
                    }
                ),
                severity: "Fout".into(),
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn cube(offset: f32) -> Mesh {
        let vertices = vec![
            [0., 0., 0., 0., 0., 1.],
            [1., 0., 0., 0., 0., 1.],
            [1., 1., 0., 0., 0., 1.],
            [0., 1., 0., 0., 0., 1.],
            [0., 0., 1., 0., 0., 1.],
            [1., 0., 1., 0., 0., 1.],
            [1., 1., 1., 0., 0., 1.],
            [0., 1., 1., 0., 0., 1.],
        ]
        .into_iter()
        .map(|mut v| {
            v[0] += offset;
            v
        })
        .collect();
        Mesh {
            id: 42,
            origin: DVec3::ZERO,
            vertices,
            indices: vec![
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2,
                7, 6, 3, 0, 4, 3, 4, 7,
            ],
            min: DVec3::new(offset as f64, 0., 0.),
            max: DVec3::new((offset + 1.) as f64, 1., 1.),
            color: [1.; 3],
        }
    }
    #[test]
    fn intersecting_meshes_report_both_model_scoped_objects() {
        let r = check(vec![((0, 42), cube(0.)), ((1, 42), cube(0.5))], 0.001);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].element, Some(42));
        assert_eq!(r[0].related, Some((1, 42)));
    }
    #[test]
    fn millimetre_overlap_survives_georeferencing() {
        let offset = DVec3::new(75032.45, 454268.5, 100.);
        let mut a = cube(0.);
        let mut b = cube(0.999);
        let local = check(vec![((0, 42), a.clone()), ((1, 42), b.clone())], 0.0001);
        for m in [&mut a, &mut b] {
            m.origin += offset;
            m.min += offset;
            m.max += offset;
        }
        let world = check(vec![((0, 42), a), ((1, 42), b)], 0.0001);
        assert!(!local.is_empty());
        assert_eq!(world.len(), local.len());
    }
    #[test]
    fn clearance_finds_gap_and_empty_b_does_not_self_clash() {
        let meshes = vec![((0, 42), cube(0.)), ((1, 42), cube(1.025))];
        let rule = Rule {
            clearance: true,
            distance: 0.05,
            ..Rule::default()
        };
        let r = check_rule(meshes.clone(), &[0], Some(&[1]), &rule);
        assert_eq!(r.len(), 1);
        assert!(r[0].rule.contains("Vrije ruimte"));
        assert!(check_rule(meshes, &[0, 1], Some(&[]), &rule).is_empty());
    }
    #[test]
    fn separated_meshes_do_not_clash() {
        assert!(check(vec![((0, 42), cube(0.)), ((1, 42), cube(2.))], 0.001).is_empty());
    }
}
