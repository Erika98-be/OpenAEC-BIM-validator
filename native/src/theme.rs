use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
pub const AMBER: Color32 = Color32::from_rgb(217, 119, 6);
pub const FORGE: Color32 = Color32::from_rgb(54, 54, 62);
pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let border = if dark {
        Color32::from_rgb(73, 73, 82)
    } else {
        Color32::from_rgb(223, 224, 226)
    };
    let surface = if dark {
        Color32::from_rgb(45, 45, 53)
    } else {
        Color32::from_rgb(250, 250, 249)
    };
    v.panel_fill = surface;
    v.window_fill = surface;
    v.faint_bg_color = if dark {
        FORGE
    } else {
        Color32::from_rgb(243, 244, 245)
    };
    v.extreme_bg_color = if dark {
        Color32::from_rgb(38, 38, 45)
    } else {
        Color32::WHITE
    };
    v.override_text_color = Some(if dark {
        Color32::from_rgb(244, 244, 243)
    } else {
        FORGE
    });
    v.selection.bg_fill = if dark {
        Color32::from_rgb(86, 63, 36)
    } else {
        Color32::from_rgb(252, 232, 201)
    };
    v.selection.stroke = Stroke::new(1.0_f32, AMBER);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
    v.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, if dark { Color32::LIGHT_GRAY } else { FORGE });
    v.widgets.inactive.bg_fill = if dark {
        Color32::from_rgb(77, 77, 87)
    } else {
        Color32::from_rgb(210, 214, 219)
    };
    v.widgets.inactive.weak_bg_fill = surface;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border);
    v.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(65, 65, 73)
    } else {
        Color32::from_rgb(235, 237, 239)
    };
    v.widgets.hovered.weak_bg_fill = v.widgets.hovered.bg_fill;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, border);
    v.widgets.active.bg_fill = v.selection.bg_fill;
    v.widgets.active.weak_bg_fill = v.selection.bg_fill;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, AMBER);
    v.hyperlink_color = AMBER;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6., 4.);
        s.spacing.button_padding = egui::vec2(9., 5.);
        s.spacing.indent = 13.;
        s.spacing.interact_size.y = 25.;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(12.));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(12.));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.));
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(16., FontFamily::Name("heading".into())),
        );
        s.text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(11.));
    });
}

pub fn fonts(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    f.font_data.insert(
        "Inter".into(),
        FontData::from_static(include_bytes!("../assets/Inter-Regular.ttf")).into(),
    );
    f.font_data.insert(
        "Space Grotesk".into(),
        FontData::from_static(include_bytes!("../assets/SpaceGrotesk-Bold.ttf")).into(),
    );
    f.font_data.insert(
        "JetBrains Mono".into(),
        FontData::from_static(include_bytes!("../assets/JetBrainsMono-Regular.ttf")).into(),
    );
    f.families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Inter".into());
    f.families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "JetBrains Mono".into());
    f.families.insert(
        FontFamily::Name("heading".into()),
        vec!["Space Grotesk".into(), "Inter".into()],
    );
    ctx.set_fonts(f);
}
