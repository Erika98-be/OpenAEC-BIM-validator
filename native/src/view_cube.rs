//! Camera-aligned orientation control. Independently implemented in egui;
//! interaction/design reference: Open Pointcloud Studio's bevelled view cube.
use crate::{camera::Camera, theme};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use glam::Vec3;

struct Facet {
    normal: Vec3,
    vertices: Vec<Vec3>,
    label: &'static str,
}
fn facets() -> Vec<Facet> {
    let mut result = Vec::new();
    let h = 0.78;
    for axis in 0..3 {
        for sign in [-1., 1.] {
            let mut normal = Vec3::ZERO;
            normal[axis] = sign;
            let vertices = [(-h, -h), (h, -h), (h, h), (-h, h)]
                .map(|(a, b)| {
                    let mut p = normal;
                    p[(axis + 1) % 3] = a;
                    p[(axis + 2) % 3] = b;
                    p
                })
                .to_vec();
            let label = match (axis, sign > 0.) {
                (0, true) => "RECHTS",
                (0, false) => "LINKS",
                (1, true) => "ACHTER",
                (1, false) => "VOOR",
                (2, true) => "BOVEN",
                _ => "ONDER",
            };
            result.push(Facet {
                normal,
                vertices,
                label,
            });
        }
    }
    for free in 0..3 {
        let a = (free + 1) % 3;
        let b = (free + 2) % 3;
        for sa in [-1., 1.] {
            for sb in [-1., 1.] {
                let mut normal = Vec3::ZERO;
                normal[a] = sa;
                normal[b] = sb;
                let vertices = [(1., h, -h), (h, 1., -h), (h, 1., h), (1., h, h)]
                    .map(|(u, v, w)| {
                        let mut p = Vec3::ZERO;
                        p[a] = u * sa;
                        p[b] = v * sb;
                        p[free] = w;
                        p
                    })
                    .to_vec();
                result.push(Facet {
                    normal: normal.normalize(),
                    vertices,
                    label: "",
                });
            }
        }
    }
    for x in [-1., 1.] {
        for y in [-1., 1.] {
            for z in [-1., 1.] {
                let sign = Vec3::new(x, y, z);
                result.push(Facet {
                    normal: sign.normalize(),
                    vertices: vec![
                        Vec3::new(1., h, h) * sign,
                        Vec3::new(h, 1., h) * sign,
                        Vec3::new(h, h, 1.) * sign,
                    ],
                    label: "",
                });
            }
        }
    }
    result
}
fn contains(points: &[Pos2], p: Pos2) -> bool {
    let mut positive = false;
    let mut negative = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        positive |= cross > 0.001;
        negative |= cross < -0.001;
    }
    !(positive && negative)
}
pub fn show(ui: &mut egui::Ui, rect: Rect, camera: &Camera) -> Option<Vec3> {
    let response = ui.interact(rect, ui.id().with("orientation-cube"), egui::Sense::click());
    let toward = camera.direction();
    let right = Vec3::Z.cross(toward).normalize();
    let up = toward.cross(right);
    let center = rect.center() + egui::vec2(0., -9.);
    let project = |p: Vec3| center + egui::vec2(p.dot(right), -p.dot(up)) * 30.;
    let painter = ui.painter();
    let ring: Vec<_> = (0..=64)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 64.;
            project(Vec3::new(angle.cos() * 1.7, angle.sin() * 1.7, -1.3))
        })
        .collect();
    painter.add(egui::Shape::line(
        ring,
        Stroke::new(1_f32, ui.visuals().weak_text_color().gamma_multiply(0.45)),
    ));
    for (text, p) in [
        ("N", Vec3::new(0., 1.95, -1.3)),
        ("Z", Vec3::new(0., -1.95, -1.3)),
        ("O", Vec3::new(1.95, 0., -1.3)),
        ("W", Vec3::new(-1.95, 0., -1.3)),
    ] {
        painter.text(
            project(p),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(9.),
            ui.visuals().weak_text_color(),
        );
    }
    let mut target = None;
    for facet in facets()
        .into_iter()
        .filter(|f| f.normal.dot(toward) > 0.025)
    {
        let points: Vec<_> = facet.vertices.iter().map(|p| project(*p)).collect();
        let hovered =
            response.hovered() && response.hover_pos().is_some_and(|p| contains(&points, p));
        let shade = (facet.normal.dot(Vec3::new(-0.3, -0.5, 0.8).normalize()) * 15.) as i16;
        let base = if ui.visuals().dark_mode { 100 } else { 222 };
        let value = (base + shade).clamp(0, 255) as u8;
        let fill = if hovered {
            theme::AMBER
        } else if facet.normal.z > 0.9 {
            Color32::from_rgb(value, value.saturating_sub(9), value.saturating_sub(23))
        } else {
            Color32::from_gray(value)
        };
        let middle = points
            .iter()
            .fold(egui::Vec2::ZERO, |sum, p| sum + p.to_vec2())
            / points.len() as f32;
        painter.add(egui::Shape::convex_polygon(
            points,
            fill,
            Stroke::new(
                0.8_f32,
                Color32::from_gray(if ui.visuals().dark_mode { 55 } else { 160 }),
            ),
        ));
        if !facet.label.is_empty() && facet.normal.dot(toward) > 0.3 {
            painter.text(
                middle.to_pos2(),
                egui::Align2::CENTER_CENTER,
                facet.label,
                egui::FontId::proportional(9.),
                if ui.visuals().dark_mode {
                    Color32::WHITE
                } else {
                    Color32::from_gray(50)
                },
            );
        }
        if hovered {
            target = Some(facet.normal);
        }
    }
    if target.is_some() {
        response
            .clone()
            .on_hover_text("Klik op een vlak, rand of hoek om het aanzicht te kiezen");
    }
    if response.clicked() {
        target
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cube_has_six_faces_twelve_edges_eight_corners() {
        let f = facets();
        assert_eq!(f.len(), 26);
        assert_eq!(f.iter().filter(|f| !f.label.is_empty()).count(), 6);
        for f in f {
            assert!((f.normal.length() - 1.).abs() < 1e-6);
        }
    }
    #[test]
    fn hit_test_rejects_empty_area_inside_bounding_rectangle() {
        let polygon = [
            egui::pos2(0., 1.),
            egui::pos2(1., 0.),
            egui::pos2(2., 1.),
            egui::pos2(1., 2.),
        ];
        assert!(contains(&polygon, egui::pos2(1., 1.)));
        assert!(!contains(&polygon, egui::pos2(0.1, 0.1)));
    }
}
