//! Local point-cloud layers. Parsers come from Open Pointcloud Studio's Rust core.
use glam::{DMat3, DMat4, DVec3};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
pub struct LayerState {
    pub path: PathBuf,
    pub transform: DMat4,
    pub visible: bool,
}
#[derive(Clone)]
pub struct Cloud {
    pub state: LayerState,
    pub points: Vec<DVec3>,
    pub colors: Vec<[f32; 3]>,
    pub total: u64,
    pub distances: Vec<f64>,
}
impl Cloud {
    pub fn load(path: &Path) -> Result<Self, String> {
        let cloud = pointcloud_core::open(path, 2_000_000).map_err(|e| e.to_string())?;
        if cloud.points.is_empty() {
            return Err("Geen punten in bestand".into());
        }
        let colors = cloud
            .points
            .iter()
            .map(|p| {
                p.rgb
                    .map(|c| c.map(|v| v as f32 / 255.))
                    .unwrap_or([0.45, 0.65, 0.8])
            })
            .collect();
        let points = cloud
            .points
            .iter()
            .map(|p| DVec3::from_array(p.xyz))
            .collect();
        Ok(Self {
            state: LayerState {
                path: path.to_owned(),
                transform: DMat4::IDENTITY,
                visible: true,
            },
            points,
            colors,
            total: cloud.total_points,
            distances: vec![],
        })
    }
    pub fn world_points(&self) -> impl Iterator<Item = DVec3> + '_ {
        self.points
            .iter()
            .map(|p| self.state.transform.transform_point3(*p))
    }
    pub fn bounds(&self) -> (DVec3, DVec3) {
        self.world_points().fold(
            (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min, max), p| (min.min(p), max.max(p)),
        )
    }
}
/// Match two non-collinear three-point frames without scale or reflection.
pub fn align(source: [DVec3; 3], target: [DVec3; 3]) -> Result<DMat4, String> {
    let basis = |p: [DVec3; 3]| -> Result<DMat3, String> {
        if !p.iter().all(|p| p.is_finite()) {
            return Err("Ongeldige registratiepunten".into());
        }
        let x = p[1] - p[0];
        let z = x.cross(p[2] - p[0]);
        if x.length() < 1e-8 || z.length() < 1e-8 {
            return Err("Registratiepunten moeten verschillend en niet collineair zijn".into());
        }
        let x = x.normalize();
        let z = z.normalize();
        Ok(DMat3::from_cols(x, z.cross(x), z))
    };
    let rotation = basis(target)? * basis(source)?.transpose();
    let translation = target[0] - rotation * source[0];
    Ok(DMat4::from_cols(
        rotation.x_axis.extend(0.),
        rotation.y_axis.extend(0.),
        rotation.z_axis.extend(0.),
        translation.extend(1.),
    ))
}
#[derive(Clone)]
struct Triangle {
    v: [DVec3; 3],
    min: DVec3,
    max: DVec3,
}
struct Node {
    min: DVec3,
    max: DVec3,
    children: Option<(Box<Node>, Box<Node>)>,
    triangles: Vec<Triangle>,
}
fn nearest_triangle(p: DVec3, v: [DVec3; 3]) -> DVec3 {
    // Ericson's Voronoi-region closest-point construction.
    let [a, b, c] = v;
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denominator = 1. / (va + vb + vc);
    a + ab * (vb * denominator) + ac * (vc * denominator)
}
impl Node {
    fn build(mut triangles: Vec<Triangle>) -> Self {
        let (min, max) = triangles.iter().fold(
            (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min, max), t| (min.min(t.min), max.max(t.max)),
        );
        if triangles.len() <= 8 {
            return Self {
                min,
                max,
                triangles,
                children: None,
            };
        }
        let extent = max - min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };
        triangles.sort_unstable_by(|a, b| {
            (a.min[axis] + a.max[axis]).total_cmp(&(b.min[axis] + b.max[axis]))
        });
        let rest = triangles.split_off(triangles.len() / 2);
        Self {
            min,
            max,
            triangles: vec![],
            children: Some((
                Box::new(Self::build(triangles)),
                Box::new(Self::build(rest)),
            )),
        }
    }
    fn distance(&self, p: DVec3) -> f64 {
        p.distance_squared(p.clamp(self.min, self.max))
    }
    fn nearest(&self, p: DVec3, best: &mut (f64, DVec3)) {
        if self.distance(p) > best.0 {
            return;
        }
        if let Some((a, b)) = &self.children {
            let (first, second) = if a.distance(p) < b.distance(p) {
                (a, b)
            } else {
                (b, a)
            };
            first.nearest(p, best);
            second.nearest(p, best);
        } else {
            for t in &self.triangles {
                let q = nearest_triangle(p, t.v);
                let d = p.distance_squared(q);
                if d < best.0 {
                    *best = (d, q);
                }
            }
        }
    }
}
pub struct Surface {
    root: Node,
}
impl Surface {
    pub fn from_meshes(meshes: impl Iterator<Item = crate::model::Mesh>) -> Result<Self, String> {
        let mut triangles = vec![];
        for mesh in meshes {
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v = tri.map(|i| {
                    DVec3::new(
                        mesh.vertices[i as usize][0] as f64,
                        mesh.vertices[i as usize][1] as f64,
                        mesh.vertices[i as usize][2] as f64,
                    ) + mesh.origin
                });
                if (v[1] - v[0]).cross(v[2] - v[0]).length_squared() < 1e-20 {
                    continue;
                }
                triangles.push(Triangle {
                    v,
                    min: v[0].min(v[1]).min(v[2]),
                    max: v[0].max(v[1]).max(v[2]),
                });
            }
        }
        if triangles.is_empty() {
            return Err("Geen IFC-oppervlakken voor afwijkingscontrole".into());
        }
        Ok(Self {
            root: Node::build(triangles),
        })
    }
    pub fn nearest(&self, p: DVec3) -> (f64, DVec3) {
        let mut best = (f64::INFINITY, DVec3::ZERO);
        self.root.nearest(p, &mut best);
        (best.0.sqrt(), best.1)
    }
    pub fn deviations(&self, cloud: &Cloud) -> Vec<f64> {
        cloud.world_points().map(|p| self.nearest(p).0).collect()
    }
}
/// Least-squares proper rigid transform, used by ICP (no scale change).
fn rigid_fit(pairs: &[(DVec3, DVec3)]) -> Result<DMat4, String> {
    if pairs.len() < 3 {
        return Err("Minder dan drie betrouwbare correspondenties".into());
    }
    let n = pairs.len() as f64;
    let source = pairs.iter().map(|p| p.0).sum::<DVec3>() / n;
    let target = pairs.iter().map(|p| p.1).sum::<DVec3>() / n;
    let mut covariance = nalgebra::Matrix3::<f64>::zeros();
    for (a, b) in pairs {
        let a = nalgebra::Vector3::from_column_slice(&(*a - source).to_array());
        let b = nalgebra::Vector3::from_column_slice(&(*b - target).to_array());
        covariance += a * b.transpose();
    }
    let svd = covariance.svd(true, true);
    if svd.singular_values[1] < 1e-12 {
        return Err("Correspondenties liggen op één lijn".into());
    }
    let u = svd.u.unwrap();
    let v = svd.v_t.unwrap().transpose();
    let mut correction = nalgebra::Matrix3::<f64>::identity();
    correction[(2, 2)] = (v * u.transpose()).determinant().signum();
    let r = v * correction * u.transpose();
    let rotation = DMat3::from_cols_array(r.as_slice().try_into().unwrap());
    Ok(DMat4::from_cols(
        rotation.x_axis.extend(0.),
        rotation.y_axis.extend(0.),
        rotation.z_axis.extend(0.),
        (target - rotation * source).extend(1.),
    ))
}
impl Surface {
    /// Refine an already roughly aligned cloud; reject distant correspondences.
    pub fn refine(&self, cloud: &mut Cloud, max_distance: f64) -> Result<(), String> {
        let stride = (cloud.points.len() / 5000).max(1);
        let original = cloud.state.transform;
        for _ in 0..60 {
            let mut matches: Vec<_> = cloud
                .world_points()
                .step_by(stride)
                .filter_map(|p| {
                    let (d, q) = self.nearest(p);
                    (d <= max_distance).then_some((d, p, q))
                })
                .collect();
            matches.sort_by(|a, b| a.0.total_cmp(&b.0));
            let keep = if matches.len() < 30 {
                matches.len()
            } else {
                matches.len() * 9 / 10
            };
            let pairs: Vec<_> = matches[..keep].iter().map(|(_, p, q)| (*p, *q)).collect();
            let step = match rigid_fit(&pairs) {
                Ok(t) => t,
                Err(e) => {
                    cloud.state.transform = original;
                    return Err(e);
                }
            };
            let before = pairs
                .iter()
                .map(|(a, b)| a.distance_squared(*b))
                .sum::<f64>();
            let after = pairs
                .iter()
                .map(|(a, b)| step.transform_point3(*a).distance_squared(*b))
                .sum::<f64>();
            if after > before + 1e-10 {
                break;
            }
            cloud.state.transform = step * cloud.state.transform;
            if before - after < 1e-10 {
                break;
            }
        }
        cloud.distances.clear();
        Ok(())
    }
}
pub struct Hotspot {
    pub center: DVec3,
    pub count: usize,
    pub max: f64,
}
impl Cloud {
    pub fn hotspots(&self, threshold: f64) -> Vec<Hotspot> {
        use std::collections::{HashMap, VecDeque};
        let mut cells: HashMap<(i64, i64, i64), (DVec3, usize, f64)> = HashMap::new();
        for (p, d) in self
            .world_points()
            .zip(&self.distances)
            .filter(|(_, d)| **d > threshold)
        {
            let key = (
                (p.x / 0.5).floor() as i64,
                (p.y / 0.5).floor() as i64,
                (p.z / 0.5).floor() as i64,
            );
            let cell = cells.entry(key).or_insert((DVec3::ZERO, 0, 0.));
            cell.0 += p;
            cell.1 += 1;
            cell.2 = cell.2.max(*d);
        }
        let mut result = vec![];
        while let Some(start) = cells.keys().next().copied() {
            let mut queue = VecDeque::from([start]);
            let mut sum = DVec3::ZERO;
            let mut count = 0;
            let mut max = 0_f64;
            while let Some(key) = queue.pop_front() {
                if let Some((p, n, d)) = cells.remove(&key) {
                    sum += p;
                    count += n;
                    max = max.max(d);
                    for delta in [
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ] {
                        let neighbor = (key.0 + delta.0, key.1 + delta.1, key.2 + delta.2);
                        if cells.contains_key(&neighbor) {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
            result.push(Hotspot {
                center: sum / count as f64,
                count,
                max,
            });
        }
        result.sort_by(|a, b| {
            b.max
                .total_cmp(&a.max)
                .then_with(|| a.center.x.total_cmp(&b.center.x))
                .then_with(|| a.center.y.total_cmp(&b.center.y))
                .then_with(|| a.center.z.total_cmp(&b.center.z))
        });
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icp_reduces_known_small_offset_against_three_planes() {
        let verts = [DVec3::ZERO, DVec3::X * 2., DVec3::Y * 2., DVec3::Z * 2.];
        let mesh = crate::model::Mesh {
            id: 1,
            origin: DVec3::ZERO,
            vertices: verts
                .map(|p| [p.x as f32, p.y as f32, p.z as f32, 0., 0., 1.])
                .to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3, 0, 3, 1],
            min: DVec3::ZERO,
            max: DVec3::splat(2.),
            color: [1.; 3],
        };
        let surface = Surface::from_meshes(std::iter::once(mesh)).unwrap();
        let mut points = vec![];
        for axis in 0..3 {
            for (a, b) in [(0.3, 0.3), (0.8, 0.4), (0.4, 0.8), (0.6, 0.6)] {
                let mut p = DVec3::ZERO;
                p[(axis + 1) % 3] = a;
                p[(axis + 2) % 3] = b;
                points.push(p + DVec3::new(0.02, 0.03, 0.01));
            }
        }
        let mut cloud = Cloud {
            state: LayerState {
                path: "test.xyz".into(),
                transform: DMat4::IDENTITY,
                visible: true,
            },
            colors: vec![[1.; 3]; points.len()],
            total: points.len() as u64,
            points,
            distances: vec![],
        };
        let before = surface.deviations(&cloud).iter().sum::<f64>();
        surface.refine(&mut cloud, 0.1).unwrap();
        let after = surface.deviations(&cloud).iter().sum::<f64>();
        assert!(
            after < before * 0.05,
            "before {before}, after {after}, transform {:?}",
            cloud.state.transform
        );
    }
    #[test]
    fn least_squares_alignment_recovers_rotation_and_translation() {
        let transform =
            DMat4::from_translation(DVec3::new(3., -4., 2.)) * DMat4::from_rotation_z(0.4);
        let pairs: Vec<_> = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z, DVec3::ONE]
            .into_iter()
            .map(|p| (p, transform.transform_point3(p)))
            .collect();
        let fit = rigid_fit(&pairs).unwrap();
        for (p, q) in pairs {
            assert!(fit.transform_point3(p).distance(q) < 1e-8);
        }
    }
    #[test]
    fn reference_scan_loads_world_coordinates_and_colors() {
        let cloud = Cloud::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/reference-scan.xyz"
        )))
        .unwrap();
        assert_eq!(cloud.points.len(), 4);
        assert_eq!(cloud.total, 4);
        assert!((cloud.points[0].x - 75032.45).abs() < 1e-9);
        assert_eq!(cloud.colors[0], [1., 0., 0.]);
        let (min, max) = cloud.bounds();
        assert!((max - min - DVec3::new(1., 1., 0.)).length() < 1e-9);
    }
    #[test]
    fn three_points_recover_rigid_transform_and_reject_line() {
        let s = [DVec3::ZERO, DVec3::X, DVec3::Y];
        let t =
            DMat4::from_translation(DVec3::new(75000., 450000., 10.)) * DMat4::from_rotation_z(0.7);
        let found = align(s, s.map(|p| t.transform_point3(p))).unwrap();
        for p in s {
            assert!(found.transform_point3(p).distance(t.transform_point3(p)) < 1e-9);
        }
        assert!(align([DVec3::ZERO, DVec3::X, DVec3::X * 2.], s).is_err());
    }
    #[test]
    fn closest_point_on_surface_and_outside_edge() {
        let t = [DVec3::ZERO, DVec3::X, DVec3::Y];
        assert!(
            nearest_triangle(DVec3::new(0.2, 0.3, 4.), t).distance(DVec3::new(0.2, 0.3, 0.)) < 1e-9
        );
        assert_eq!(nearest_triangle(DVec3::new(-2., 0., 0.), t), DVec3::ZERO);
    }
}
