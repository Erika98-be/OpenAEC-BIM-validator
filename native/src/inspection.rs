//! Geometry inspection helpers, independent of UI and world origin.
use crate::{
    camera::{ray_box, ray_triangle},
    model::Mesh,
    renderer::Key,
    validation::Finding,
};
use glam::DVec3;
pub fn polygon_area(points: &[DVec3]) -> Result<f64, String> {
    if points.len() < 3 {
        return Err("Klik minstens drie hoekpunten".into());
    }
    let origin = points[0];
    let mut normal = DVec3::ZERO;
    for i in 0..points.len() {
        normal += (points[i] - origin).cross(points[(i + 1) % points.len()] - origin);
    }
    let length = normal.length();
    if length < 1e-12 {
        return Err("De punten vormen geen oppervlak".into());
    }
    let unit = normal / length;
    if points.iter().any(|p| (*p - origin).dot(unit).abs() > 0.001) {
        return Err("Hoekpunten liggen niet in één vlak (1 mm tolerantie)".into());
    }
    Ok(length * 0.5)
}
pub fn visibility(
    meshes: impl Iterator<Item = (Key, Mesh)>,
    start: DVec3,
    end: DVec3,
) -> Vec<Finding> {
    let delta = end - start;
    let distance = delta.length();
    if distance < 0.002 {
        return vec![];
    }
    let dir = (delta / distance).as_vec3();
    let mut blockers = vec![];
    for (key, mesh) in meshes {
        let origin = (start - mesh.origin).as_vec3();
        if !ray_box(
            origin,
            dir,
            (mesh.min - mesh.origin).as_vec3(),
            (mesh.max - mesh.origin).as_vec3(),
        ) {
            continue;
        }
        let nearest = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter_map(|tri| {
                let v = |i: u32| glam::Vec3::from_slice(&mesh.vertices[i as usize][..3]);
                ray_triangle(origin, dir, v(tri[0]), v(tri[1]), v(tri[2]))
                    .filter(|t| *t > 0.001 && (*t as f64) < distance - 0.001)
            })
            .min_by(f32::total_cmp);
        if let Some(t) = nearest {
            blockers.push((
                t,
                Finding {
                    model: key.0,
                    element: Some(key.1),
                    related: None,
                    rule: "Zichtlijn".into(),
                    message: format!(
                        "Zichtlijn geblokkeerd op {:.3} m van het oogpunt (totale lengte {:.3} m)",
                        t, distance
                    ),
                    severity: "Fout".into(),
                },
            ));
        }
    }
    blockers.sort_by(|a, b| a.0.total_cmp(&b.0));
    blockers.into_iter().map(|(_, f)| f).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn area_survives_georeferencing_and_rejects_nonplanar_points() {
        let offset = DVec3::new(75032.4, 454268.8, 4.);
        let mut p = vec![
            DVec3::ZERO,
            DVec3::new(3., 0., 0.),
            DVec3::new(3., 2., 0.),
            DVec3::new(0., 2., 0.),
        ];
        for point in &mut p {
            *point += offset;
        }
        assert!((polygon_area(&p).unwrap() - 6.).abs() < 1e-6);
        p[2].z += 0.01;
        assert!(polygon_area(&p).is_err());
    }
    #[test]
    fn visibility_reports_intermediate_triangle_but_not_geometry_beyond_target() {
        let mesh = Mesh {
            id: 1,
            origin: DVec3::ZERO,
            vertices: vec![
                [1., -1., -1., 1., 0., 0.],
                [1., 1., -1., 1., 0., 0.],
                [1., 0., 1., 1., 0., 0.],
            ],
            indices: vec![0, 1, 2],
            min: DVec3::new(1., -1., -1.),
            max: DVec3::new(1., 1., 1.),
            color: [1.; 3],
        };
        assert_eq!(
            visibility(
                std::iter::once(((0, 1), mesh.clone())),
                DVec3::ZERO,
                DVec3::new(2., 0., 0.)
            )
            .len(),
            1
        );
        assert!(visibility(
            std::iter::once(((0, 1), mesh)),
            DVec3::ZERO,
            DVec3::new(0.5, 0., 0.)
        )
        .is_empty());
    }
}
