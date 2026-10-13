use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub perspective: bool,
    pub fov: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 20.,
            yaw: -0.8,
            pitch: 0.55,
            perspective: true,
            fov: 45.,
        }
    }
}
impl Camera {
    pub fn direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
        )
    }
    pub fn eye(&self) -> Vec3 {
        self.target + self.direction() * self.distance
    }
    #[cfg(test)]
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        self.relative_matrix(aspect, Vec3::ZERO)
    }
    /// Subtract the reference before constructing the view matrix. Applying a
    /// world-space f32 matrix to georeferenced vertices loses depth precision.
    pub fn relative_matrix(&self, aspect: f32, reference: Vec3) -> Mat4 {
        let near = (self.distance * 0.01).max(0.00001);
        let far = self.distance * 20.;
        let proj = if self.perspective {
            Mat4::perspective_rh_gl(self.fov.to_radians(), aspect, near, far)
        } else {
            let h = self.distance * (self.fov.to_radians() * 0.5).tan();
            Mat4::orthographic_rh_gl(-h * aspect, h * aspect, -h, h, near, far)
        };
        let target = self.target - reference;
        let eye = target + self.direction() * self.distance;
        proj * Mat4::look_at_rh(eye, target, Vec3::Z)
    }
    /// Fit projected bounds to the available viewport, including narrow panels.
    pub fn fit(&mut self, min: Vec3, max: Vec3, aspect: f32) {
        self.target = (min + max) * 0.5;
        let points = [min.x, max.x].into_iter().flat_map(|x| {
            [min.y, max.y]
                .into_iter()
                .flat_map(move |y| [min.z, max.z].into_iter().map(move |z| Vec3::new(x, y, z)))
        });
        self.fit_points(points, aspect);
    }
    /// A world-aligned box overestimates a rotated or irregular building.
    /// Retain the target while fitting the actual referenced mesh vertices.
    pub fn fit_points(&mut self, points: impl Iterator<Item = Vec3>, aspect: f32) {
        let direction = self.direction();
        let right = Vec3::Z.cross(direction).normalize();
        let up = direction.cross(right);
        let tan_y = (self.fov.to_radians() * 0.5).tan();
        let tan_x = tan_y * aspect.max(0.1);
        let mut distance = 0.1_f32;
        for point in points {
            let p = point - self.target;
            let depth = if self.perspective {
                p.dot(direction)
            } else {
                0.
            };
            distance = distance
                .max(depth + p.dot(right).abs() / tan_x)
                .max(depth + p.dot(up).abs() / tan_y);
        }
        self.distance = (distance * 1.18).max(0.1);
    }
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.56, 1.56);
    }
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let right = Vec3::Z.cross(self.direction()).normalize();
        let up = self.direction().cross(right);
        let scale = 2. * self.distance * (self.fov.to_radians() * 0.5).tan() / height.max(1.);
        self.target += (-right * dx + up * dy) * scale;
    }
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (-delta * 0.002).exp()).clamp(0.001, 1e10);
    }
    /// Ray origin relative to `target`, avoiding large-coordinate cancellation.
    pub fn ray(&self, x: f32, y: f32, aspect: f32) -> (Vec3, Vec3) {
        let backward = self.direction();
        let right = Vec3::Z.cross(backward).normalize();
        let up = backward.cross(right);
        let half_height = (self.fov.to_radians() * 0.5).tan();
        let offset = (right * x * aspect + up * y) * half_height;
        let eye = backward * self.distance;
        if self.perspective {
            (eye, (offset - backward).normalize())
        } else {
            (eye + offset * self.distance, -backward)
        }
    }
}
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1. / det;
    let s = origin - a;
    let u = inv * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = inv * dir.dot(q);
    if v < 0. || u + v > 1. {
        return None;
    }
    let t = inv * e2.dot(q);
    (t > 0.).then_some(t)
}
pub fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> bool {
    let mut near: f32 = 0.;
    let mut far = f32::INFINITY;
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if origin[i] < min[i] || origin[i] > max[i] {
                return false;
            }
        } else {
            let a = (min[i] - origin[i]) / dir[i];
            let b = (max[i] - origin[i]) / dir[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_center_ray_hits_target() {
        let c = Camera::default();
        let (o, d) = c.ray(0., 0., 1.);
        assert!((o + d * (c.target - o).dot(d) - c.target).length() < 0.01);
    }
    #[test]
    fn fit_keeps_all_corners_inside_wide_and_narrow_viewports() {
        for aspect in [0.5, 1., 2.5] {
            for perspective in [true, false] {
                let mut c = Camera {
                    perspective,
                    ..Camera::default()
                };
                let min = Vec3::new(-22., -7., -3.);
                let max = Vec3::new(22., 7., 12.);
                c.fit(min, max, aspect);
                for x in [min.x, max.x] {
                    for y in [min.y, max.y] {
                        for z in [min.z, max.z] {
                            let p = c.matrix(aspect).project_point3(Vec3::new(x, y, z));
                            assert!(p.x.abs() < 1. && p.y.abs() < 1. && p.z.abs() < 1., "{p:?}");
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn georeferenced_projection_and_picking_preserve_local_detail() {
        for perspective in [true, false] {
            let c = Camera {
                target: Vec3::new(75032., 454268., 12.),
                perspective,
                ..Camera::default()
            };
            let p = Vec3::new(0.125, 0.007, 0.5);
            let matrix = c.relative_matrix(1.8, c.target);
            let near_origin = Camera {
                target: Vec3::ZERO,
                ..c.clone()
            }
            .relative_matrix(1.8, Vec3::ZERO);
            assert!((matrix.project_point3(p) - near_origin.project_point3(p)).length() < 1e-6);
            let screen = matrix.project_point3(p);
            let (o, d) = c.ray(screen.x, screen.y, 1.8);
            assert!((o + d * (p - o).dot(d) - p).length() < 0.0001);
        }
    }
    #[test]
    fn triangle_hit_and_miss() {
        let a = Vec3::ZERO;
        let b = Vec3::X;
        let c = Vec3::Y;
        assert_eq!(
            ray_triangle(Vec3::new(0.2, 0.2, 2.), -Vec3::Z, a, b, c),
            Some(2.)
        );
        assert!(ray_triangle(Vec3::new(2., 2., 2.), -Vec3::Z, a, b, c).is_none());
    }
    #[test]
    fn parallel_box_ray() {
        assert!(!ray_box(
            Vec3::new(2., 0., 2.),
            -Vec3::Z,
            Vec3::ZERO,
            Vec3::ONE
        ));
        assert!(ray_box(
            Vec3::new(0.5, 0.5, 2.),
            -Vec3::Z,
            Vec3::ZERO,
            Vec3::ONE
        ));
    }
}
