//! Compact, resolution-independent controls for the OpenAEC desktop shell.
use crate::theme;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Response, Sense, Stroke};

#[derive(Clone, Copy)]
pub enum Icon {
    Cube,
    Isolate,
    Clash,
    SectionBox,
    Transparent,
    Orbit,
    Fit,
    Wire,
    Eye,
    Hide,
    Slice,
    Measure,
    Folder,
    Save,
    Check,
    List,
    Settings,
    Bookmark,
    Close,
    Layers,
    Sun,
}

pub fn icon(p: &egui::Painter, rect: Rect, kind: Icon, color: Color32) {
    let at = |x: f32, y: f32| {
        pos2(
            rect.left() + x * rect.width() / 24.,
            rect.top() + y * rect.height() / 24.,
        )
    };
    let stroke = Stroke::new(1.4_f32, color);
    let path = |points: &[[f32; 2]]| {
        p.add(egui::Shape::line(
            points.iter().map(|v| at(v[0], v[1])).collect(),
            stroke,
        ));
    };
    match kind {
        Icon::Isolate => {
            path(&[[3., 8.], [3., 3.], [8., 3.]]);
            path(&[[16., 3.], [21., 3.], [21., 8.]]);
            path(&[[21., 16.], [21., 21.], [16., 21.]]);
            path(&[[8., 21.], [3., 21.], [3., 16.]]);
            path(&[
                [12., 7.],
                [17., 10.],
                [17., 15.],
                [12., 18.],
                [7., 15.],
                [7., 10.],
                [12., 7.],
            ]);
            path(&[[7., 10.], [12., 13.], [17., 10.]]);
            path(&[[12., 13.], [12., 18.]]);
        }
        Icon::Clash => {
            path(&[[2., 5.], [13., 5.], [13., 16.], [2., 16.], [2., 5.]]);
            path(&[[11., 9.], [22., 9.], [22., 20.], [11., 20.], [11., 9.]]);
            path(&[[7., 1.], [9., 3.]]);
            path(&[[17., 21.], [19., 23.]]);
        }
        Icon::SectionBox => {
            path(&[
                [12., 3.],
                [21., 8.],
                [21., 18.],
                [12., 23.],
                [3., 18.],
                [3., 8.],
                [12., 3.],
            ]);
            path(&[[3., 8.], [12., 13.], [21., 8.]]);
            path(&[[12., 13.], [12., 23.]]);
            path(&[[1., 12.], [10., 7.], [23., 14.], [14., 19.], [1., 12.]]);
        }
        Icon::Transparent => {
            path(&[[2., 3.], [15., 3.], [15., 16.], [2., 16.], [2., 3.]]);
            path(&[[9., 8.], [22., 8.], [22., 21.], [9., 21.], [9., 8.]]);
            for x in [10., 13.] {
                path(&[[x, 10.], [x, 14.]]);
            }
        }

        Icon::Cube | Icon::Wire | Icon::Slice => {
            path(&[
                [12., 2.],
                [21., 7.],
                [21., 17.],
                [12., 22.],
                [3., 17.],
                [3., 7.],
                [12., 2.],
            ]);
            path(&[[3., 7.], [12., 12.], [21., 7.]]);
            path(&[[12., 12.], [12., 22.]]);
            if matches!(kind, Icon::Wire) {
                path(&[[3., 17.], [12., 12.], [21., 17.]]);
            }
            if matches!(kind, Icon::Slice) {
                path(&[[1., 13.], [23., 13.]]);
            }
        }
        Icon::Orbit => {
            p.circle_stroke(rect.center(), rect.width() * 0.36, stroke);
            path(&[[8., 2.], [5., 5.], [8., 8.]]);
            path(&[[16., 16.], [19., 19.], [16., 22.]]);
            p.circle_filled(rect.center(), 2., color);
        }
        Icon::Fit => {
            for pts in [
                &[[3., 9.], [3., 3.], [9., 3.]][..],
                &[[15., 3.], [21., 3.], [21., 9.]],
                &[[21., 15.], [21., 21.], [15., 21.]],
                &[[9., 21.], [3., 21.], [3., 15.]],
            ] {
                path(pts);
            }
        }
        Icon::Eye | Icon::Hide => {
            path(&[
                [2., 12.],
                [6., 7.],
                [12., 5.],
                [18., 7.],
                [22., 12.],
                [18., 17.],
                [12., 19.],
                [6., 17.],
                [2., 12.],
            ]);
            p.circle_stroke(rect.center(), rect.width() * 0.13, stroke);
            if matches!(kind, Icon::Hide) {
                path(&[[3., 3.], [21., 21.]]);
            }
        }
        Icon::Measure => {
            path(&[[3., 17.], [17., 3.], [22., 8.], [8., 22.], [3., 17.]]);
            path(&[[8., 12.], [11., 15.]]);
            path(&[[12., 8.], [15., 11.]]);
        }
        Icon::Folder => {
            path(&[
                [2., 6.],
                [9., 6.],
                [11., 9.],
                [22., 9.],
                [20., 20.],
                [2., 20.],
                [2., 6.],
            ]);
        }
        Icon::Save => {
            path(&[
                [3., 3.],
                [18., 3.],
                [21., 6.],
                [21., 21.],
                [3., 21.],
                [3., 3.],
            ]);
            path(&[[7., 3.], [7., 9.], [16., 9.], [16., 3.]]);
            path(&[[7., 21.], [7., 14.], [17., 14.], [17., 21.]]);
        }
        Icon::Check => {
            path(&[[3., 12.], [9., 18.], [21., 5.]]);
        }
        Icon::List => {
            for y in [5., 12., 19.] {
                p.circle_filled(at(3., y), 1., color);
                path(&[[8., y], [21., y]]);
            }
        }
        Icon::Settings => {
            for (y, x) in [(5., 8.), (12., 16.), (19., 10.)] {
                path(&[[2., y], [22., y]]);
                p.circle_filled(at(x, y), 2.5, color);
            }
        }
        Icon::Bookmark => {
            path(&[
                [5., 3.],
                [19., 3.],
                [19., 22.],
                [12., 17.],
                [5., 22.],
                [5., 3.],
            ]);
        }
        Icon::Close => {
            path(&[[6., 6.], [18., 18.]]);
            path(&[[18., 6.], [6., 18.]]);
        }
        Icon::Layers => {
            path(&[[2., 8.], [12., 3.], [22., 8.], [12., 13.], [2., 8.]]);
            path(&[[2., 13.], [12., 18.], [22., 13.]]);
            path(&[[2., 18.], [12., 23.], [22., 18.]]);
        }
        Icon::Sun => {
            p.circle_stroke(rect.center(), rect.width() * 0.2, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                let v = vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        rect.center() + v * rect.width() * 0.32,
                        rect.center() + v * rect.width() * 0.46,
                    ],
                    stroke,
                );
            }
        }
    }
}
pub fn tool(ui: &mut egui::Ui, label: &str, kind: Icon, active: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(70., 62.), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
    });
    let fill = if active {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect.shrink(1.), 4., fill);
    let color = if !ui.is_enabled() {
        ui.visuals().weak_text_color()
    } else if active {
        theme::AMBER
    } else {
        ui.visuals().text_color()
    };
    icon(
        ui.painter(),
        Rect::from_center_size(rect.center_top() + vec2(0., 20.), vec2(23., 23.)),
        kind,
        color,
    );
    ui.painter().text(
        rect.center_bottom() - vec2(0., 12.),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.),
        color,
    );
    response
}
pub fn icon_button(ui: &mut egui::Ui, kind: Icon, label: &str) -> Response {
    let (r, response) = ui.allocate_exact_size(vec2(26., 26.), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if response.hovered() {
        ui.painter()
            .rect_filled(r, 3., ui.visuals().widgets.hovered.bg_fill);
    }
    icon(ui.painter(), r.shrink(5.), kind, ui.visuals().text_color());
    response.on_hover_text(label)
}
pub fn tab(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let r = ui.add(
        egui::Button::new(egui::RichText::new(label).size(12.).color(if active {
            theme::AMBER
        } else {
            ui.visuals().text_color()
        }))
        .frame(false)
        .min_size(vec2(62., 28.)),
    );
    if active {
        ui.painter().line_segment(
            [r.rect.left_bottom(), r.rect.right_bottom()],
            Stroke::new(2.0_f32, theme::AMBER),
        );
    }
    r
}
pub fn group(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let r = ui.horizontal(body).response;
        ui.allocate_ui_with_layout(
            vec2(r.rect.width(), 15.),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(egui::RichText::new(label).size(10.).weak());
            },
        );
    });
    ui.separator();
}
pub fn section(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(label).strong().size(12.));
        });
}
