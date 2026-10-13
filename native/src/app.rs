mod data_ui;
use crate::{
    camera::{self, Camera},
    model::Model,
    renderer::{Draw, Key, Renderer},
    theme,
    validation::{self, Finding},
};
use eframe::{
    egui::{self, Color32, RichText, Sense},
    glow,
};
use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

type CloudResult = Result<(Option<usize>, crate::pointcloud::Cloud), String>;

type RibbonGroup<'a> = (&'a str, Vec<(&'a str, crate::widgets::Icon, &'a str, bool)>);

#[derive(Clone, Serialize, Deserialize)]
struct View {
    name: String,
    camera: Camera,
    hidden: HashSet<Key>,
    clip: bool,
    clip_min: Vec3,
    clip_max: Vec3,
    mode: u8,
    edges: bool,
}
#[derive(Serialize, Deserialize)]
struct Project {
    version: u32,
    models: Vec<PathBuf>,
    view: View,
    views: Vec<View>,
    #[serde(default)]
    rules: Vec<crate::clash::Rule>,
    #[serde(default)]
    topics: Vec<crate::issues::Topic>,
    #[serde(default)]
    findings: Vec<Finding>,
    #[serde(default)]
    clouds: Vec<crate::pointcloud::LayerState>,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    quality: crate::quality::Options,
    dark: bool,
    edges: bool,
    mode: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            quality: Default::default(),
            dark: false,
            edges: false,
            mode: 3,
        }
    }
}
enum Import {
    Progress(String),
    Loaded(Result<Model, String>),
    Done,
}
pub struct BimApp {
    query: crate::data::Query,
    data_column: String,
    export_height: f64,
    data_filter: bool,
    data_mode: usize,
    diff_before: usize,
    diff_after: usize,
    differences: Vec<crate::data::Difference>,
    clouds: Vec<crate::pointcloud::Cloud>,
    cloud_rx: Option<mpsc::Receiver<CloudResult>>,
    cloud_window: bool,
    cloud_index: usize,
    cloud_threshold: f64,
    registration_source: [DVec3; 3],
    registration_target: [DVec3; 3],
    topics: Vec<crate::issues::Topic>,
    topics_open: bool,
    ids_editor_open: bool,
    quality_open: bool,
    ids_source: String,
    viewport_rect: egui::Rect,
    bcf_version3: bool,
    active_topic: Option<usize>,
    topic_filter: String,
    comment_draft: String,
    rules: Vec<crate::clash::Rule>,
    rules_open: bool,
    brand: egui::TextureHandle,
    gl: Arc<glow::Context>,
    renderer: Arc<Mutex<Renderer>>,
    models: Vec<Model>,
    camera: Camera,
    viewport_aspect: f32,
    fit_pending: bool,
    selected: HashSet<Key>,
    hidden: HashSet<Key>,
    prefs: Preferences,
    tab: usize,
    file_menu: bool,
    left_open: bool,
    right_open: bool,
    result_filter: usize,
    inspector: usize,
    search: String,
    grouping: u8,
    status: String,
    error: Option<String>,
    rx: Option<mpsc::Receiver<Import>>,
    progress: String,
    clip: bool,
    clip_min: Vec3,
    clip_max: Vec3,
    bounds: (Vec3, Vec3),
    results: Vec<Finding>,
    results_open: bool,
    results_checked: bool,
    views: Vec<View>,
    view_name: String,
    measurement: bool,
    area_measurement: bool,
    walk: bool,
    points: Vec<DVec3>,
    pending_view: Option<View>,
    help: bool,
    clash_rx: Option<mpsc::Receiver<Result<Vec<Finding>, String>>>,
    clash_tolerance: f64,
    #[cfg(feature = "screenshot")]
    screenshot_requested: bool,
}
impl BimApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let prefs: Preferences = if cfg!(feature = "screenshot")
            && std::env::var_os("OPENAEC_SCREENSHOT_TO").is_some()
        {
            Preferences::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, "studio-preferences-v1"))
                .unwrap_or_default()
        };
        theme::fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, prefs.dark);
        let gl = cc.gl.clone().expect("OpenGL-context vereist");
        let renderer = Arc::new(Mutex::new(Renderer::new(&gl).expect("OpenGL 3.3 vereist")));
        let icon =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/openaec-symbol.png"))
                .expect("OpenAEC icon");
        let brand = cc.egui_ctx.load_texture(
            "openaec-brand",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        let mut app = Self {
            clouds: vec![],
            cloud_rx: None,
            cloud_window: false,
            cloud_index: 0,
            cloud_threshold: 0.05,
            registration_source: [DVec3::ZERO, DVec3::X, DVec3::Y],
            registration_target: [DVec3::ZERO, DVec3::X, DVec3::Y],
            topics: vec![],
            query: Default::default(),
            data_column: "Material".into(),
            export_height: 1.2,
            data_filter: false,
            data_mode: 0,
            diff_before: 0,
            diff_after: 1,
            differences: vec![],
            topics_open: false,
            ids_editor_open: false,
            quality_open: false,
            ids_source: include_str!("../../ids-bestanden/NL_BIM_Basis_ILS_v2.ids").into(),
            viewport_rect: egui::Rect::NOTHING,
            bcf_version3: false,
            active_topic: None,
            topic_filter: String::new(),
            comment_draft: String::new(),
            rules: vec![],
            rules_open: false,
            brand,
            gl,
            renderer,
            models: vec![],
            camera: Camera::default(),
            viewport_aspect: 1.7,
            fit_pending: false,
            selected: HashSet::new(),
            hidden: HashSet::new(),
            prefs,
            tab: 2,
            file_menu: false,
            left_open: true,
            right_open: true,
            result_filter: 0,
            inspector: 0,
            search: String::new(),
            grouping: 0,
            status: "Gereed · open een IFC-model om te beginnen".into(),
            error: None,
            rx: None,
            progress: String::new(),
            clip: false,
            clip_min: Vec3::splat(-10.),
            clip_max: Vec3::splat(10.),
            bounds: (Vec3::splat(-10.), Vec3::splat(10.)),
            results: vec![],
            results_open: true,
            results_checked: false,
            views: vec![],
            view_name: String::new(),
            measurement: false,
            area_measurement: false,
            walk: false,
            points: vec![],
            pending_view: None,
            help: false,
            clash_rx: None,
            clash_tolerance: 0.001,
            #[cfg(feature = "screenshot")]
            screenshot_requested: false,
        };
        if let Some(path) = initial {
            if cfg!(feature = "screenshot") && std::env::var_os("OPENAEC_SCREENSHOT_TO").is_some() {
                match Model::load(&path, |_| {}) {
                    Ok(model) => {
                        app.renderer.lock().unwrap().upload(&app.gl, 0, &model);
                        app.status = format!(
                            "{} · {} objecten · {} driehoeken",
                            model.name,
                            model.elements.len(),
                            model.triangle_count()
                        );
                        app.models.push(model);
                        app.tab = 2;
                        app.recalculate_bounds();
                        app.camera
                            .fit(app.bounds.0, app.bounds.1, app.viewport_aspect);
                        app.clip_min = app.bounds.0;
                        app.clip_max = app.bounds.1;
                        app.fit_pending = true;
                        #[cfg(feature = "screenshot")]
                        if std::env::var_os("OPENAEC_SCREENSHOT_REVIEW").is_some() {
                            if let Some(mesh) = app.models[0].meshes.first() {
                                app.selected.insert((0, mesh.id));
                            }
                            app.results = validation::check_models(&app.models);
                            app.results_checked = true;
                            app.results_open = true;
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref()
                                == Ok("settings")
                            {
                                app.command("slice", &cc.egui_ctx);
                            }
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref() == Ok("issues")
                            {
                                app.command("findings-to-issues", &cc.egui_ctx);
                                app.active_topic = Some(0);
                                app.tab = 4;
                            }
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref() == Ok("data") {
                                app.tab = 5;
                            }
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref() == Ok("rules")
                            {
                                app.rules.push(crate::clash::Rule {
                                    name: "Constructie versus installaties".into(),
                                    clearance: true,
                                    distance: 0.05,
                                    ..Default::default()
                                });
                                app.rules_open = true;
                                app.tab = 3;
                            }
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref() == Ok("scans")
                            {
                                let cloud =
                                    crate::pointcloud::Cloud::load(std::path::Path::new(concat!(
                                        env!("CARGO_MANIFEST_DIR"),
                                        "/tests/fixtures/reference-scan.xyz"
                                    )))
                                    .unwrap();
                                app.clouds.push(cloud);
                                app.renderer.lock().unwrap().upload_clouds(
                                    &app.gl,
                                    &app.clouds,
                                    0.05,
                                );
                                app.cloud_window = true;
                                app.tab = 6;
                            }
                            if std::env::var("OPENAEC_SCREENSHOT_REVIEW").as_deref() == Ok("dark") {
                                app.prefs.dark = true;
                                theme::apply(&cc.egui_ctx, true);
                            }
                        }
                    }
                    Err(e) => app.error = Some(e),
                }
            } else {
                app.load(vec![path], cc.egui_ctx.clone());
            }
        }
        app
    }
    fn load(&mut self, paths: Vec<PathBuf>, ctx: egui::Context) {
        if self.rx.is_some() {
            return;
        }
        self.differences.clear();
        self.clash_rx = None;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.progress = "IFC-import voorbereiden…".into();
        std::thread::spawn(move || {
            for path in paths {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Model::load(&path, |text| {
                        let _ = tx.send(Import::Progress(text));
                        ctx.request_repaint();
                    })
                }))
                .unwrap_or_else(|_| Err(format!("IfcLite kon {} niet verwerken", path.display())));
                let _ = tx.send(Import::Loaded(result));
                ctx.request_repaint();
            }
            let _ = tx.send(Import::Done);
            ctx.request_repaint();
        });
    }
    fn import_dialog(&mut self, ctx: &egui::Context) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("IFC STEP", &["ifc"])
            .pick_files()
        {
            self.load(paths, ctx.clone());
        }
    }
    fn poll(&mut self) {
        let mut cloud_messages = vec![];
        let mut cloud_done = false;
        if let Some(rx) = &self.cloud_rx {
            loop {
                match rx.try_recv() {
                    Ok(result) => cloud_messages.push(result),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        cloud_done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        for message in cloud_messages {
            match message {
                Ok((index, cloud)) => {
                    if let Some(i) = index {
                        if let Some(existing) = self.clouds.get_mut(i) {
                            let mut cloud = cloud;
                            cloud.state.visible = existing.state.visible;
                            *existing = cloud;
                        }
                    } else {
                        self.clouds.push(cloud);
                        self.cloud_index = self.clouds.len() - 1;
                    }
                    self.renderer.lock().unwrap().upload_clouds(
                        &self.gl,
                        &self.clouds,
                        self.cloud_threshold,
                    );
                    self.status = "Puntenwolk gereed".into();
                    self.cloud_window = true;
                }
                Err(error) => self.error = Some(error),
            }
        }
        if cloud_done {
            self.cloud_rx = None;
        }
        if let Some(result) = self.clash_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.clash_rx = None;
            match result {
                Ok(results) => {
                    self.status = format!("Clashcontrole gereed · {} clashes", results.len());
                    self.results = results;
                    self.results_checked = true;
                    self.results_open = true;
                }
                Err(e) => self.error = Some(e),
            }
        }
        let messages: Vec<_> = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for message in messages {
            match message {
                Import::Progress(text) => self.progress = text,
                Import::Loaded(Ok(model)) => {
                    self.status = format!(
                        "{} · {} objecten · {} driehoeken · {} importmeldingen",
                        model.name,
                        model.elements.len(),
                        model.triangle_count(),
                        model.warnings.len()
                    );
                    self.renderer
                        .lock()
                        .unwrap()
                        .upload(&self.gl, self.models.len(), &model);
                    self.models.push(model);
                    self.tab = 2;
                    self.recalculate_bounds();
                    if self.pending_view.is_none() {
                        self.fit_pending = true;
                        self.camera
                            .fit(self.bounds.0, self.bounds.1, self.viewport_aspect);
                        self.clip_min = self.bounds.0;
                        self.clip_max = self.bounds.1;
                    }
                    if self.pending_view.is_none() {
                        self.results.clear();
                        self.results_checked = false;
                    }
                }
                Import::Loaded(Err(err)) => {
                    if self.pending_view.take().is_some() {
                        self.views.clear();
                        self.rules.clear();
                        self.topics.clear();
                        self.active_topic = None;
                    }
                    self.error = Some(err);
                }
                Import::Done => {
                    self.rx = None;
                    if let Some(view) = self.pending_view.take() {
                        self.restore(view);
                    }
                }
            }
        }
    }
    fn recalculate_bounds(&mut self) {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for m in self.models.iter().flat_map(|m| &m.meshes) {
            min = min.min(m.min.as_vec3());
            max = max.max(m.max.as_vec3());
        }
        if min.is_finite() {
            self.bounds = (min, max);
        }
    }
    fn fit(&mut self, selection: bool) {
        let selected = self.expand(&self.selected);
        let hidden = self.expand(&self.hidden);
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (mi, model) in self.models.iter().enumerate() {
            for m in &model.meshes {
                let key = (mi, m.id);
                if !hidden.contains(&key) && (!selection || selected.contains(&key)) {
                    min = min.min(m.min.as_vec3());
                    max = max.max(m.max.as_vec3());
                }
            }
        }
        if min.is_finite() {
            self.camera.fit(min, max, self.viewport_aspect);
            let points = self
                .models
                .iter()
                .enumerate()
                .flat_map(|(mi, model)| model.meshes.iter().map(move |mesh| ((mi, mesh.id), mesh)))
                .filter(|(key, _)| !hidden.contains(key) && (!selection || selected.contains(key)))
                .flat_map(|(_, mesh)| {
                    mesh.indices.iter().map(|index| {
                        let v = mesh.vertices[*index as usize];
                        (Vec3::new(v[0], v[1], v[2]).as_dvec3() + mesh.origin).as_vec3()
                    })
                });
            self.camera.fit_points(points, self.viewport_aspect);
        }
    }
    fn select(&mut self, key: Key, add: bool) {
        if !add {
            self.selected.clear();
        }
        if !self.selected.insert(key) && add {
            self.selected.remove(&key);
        }
        self.inspector = 0;
    }
    fn isolate(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let selected = self.expand(&self.selected);
        self.hidden.clear();
        for (mi, m) in self.models.iter().enumerate() {
            for mesh in &m.meshes {
                let key = (mi, mesh.id);
                if !selected.contains(&key) {
                    self.hidden.insert(key);
                }
            }
        }
        self.fit(true);
    }
    fn expand(&self, keys: &HashSet<Key>) -> HashSet<Key> {
        let mut result = keys.clone();
        for (mi, model) in self.models.iter().enumerate() {
            for e in model.elements.values() {
                let mut current = Some(e.id);
                let mut seen = HashSet::new();
                while let Some(id) = current {
                    if !seen.insert(id) {
                        break;
                    }
                    if keys.contains(&(mi, id)) {
                        result.insert((mi, e.id));
                        break;
                    }
                    current = model.elements.get(&id).and_then(|e| e.parent);
                }
            }
        }
        result
    }
    fn capture(&self, name: String) -> View {
        View {
            name,
            camera: self.camera.clone(),
            hidden: self.hidden.clone(),
            clip: self.clip,
            clip_min: self.clip_min,
            clip_max: self.clip_max,
            mode: self.prefs.mode,
            edges: self.prefs.edges,
        }
    }
    fn restore(&mut self, v: View) {
        self.fit_pending = false;
        self.camera = v.camera;
        self.hidden = v.hidden;
        self.clip = v.clip;
        self.clip_min = v.clip_min;
        self.clip_max = v.clip_max;
        self.prefs.mode = v.mode.min(3);
        self.prefs.edges = v.edges;
        self.points.clear();
    }
    fn clear(&mut self) {
        self.differences.clear();
        self.clash_rx = None;
        self.cloud_rx = None;
        self.clouds.clear();
        self.renderer.lock().unwrap().clear(&self.gl);
        self.models.clear();
        self.selected.clear();
        self.hidden.clear();
        self.results.clear();
        self.results_checked = false;
        self.views.clear();
        self.rules.clear();
        self.topics.clear();
        self.active_topic = None;
        self.points.clear();
        self.clip = false;
        self.status = "Werkruimte gesloten".into();
    }
    fn save_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenAEC-project", &["oaec"])
            .set_file_name("project.oaec")
            .save_file()
        {
            let p = Project {
                version: 1,
                models: self
                    .models
                    .iter()
                    .map(|m| std::fs::canonicalize(&m.path).unwrap_or(m.path.clone()))
                    .collect(),
                view: self.capture("Actueel".into()),
                views: self.views.clone(),
                rules: self.rules.clone(),
                topics: self.topics.clone(),
                findings: self.results.clone(),
                clouds: self
                    .clouds
                    .iter()
                    .map(|c| {
                        let mut state = c.state.clone();
                        state.path = std::fs::canonicalize(&state.path).unwrap_or(state.path);
                        state
                    })
                    .collect(),
            };
            match serde_json::to_vec_pretty(&p)
                .map_err(|e| e.to_string())
                .and_then(|data| std::fs::write(path, data).map_err(|e| e.to_string()))
            {
                Ok(()) => {
                    self.status =
                        "Project opgeslagen; IFC-bestanden blijven op hun oorspronkelijke locatie"
                            .into()
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn open_project(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenAEC-project", &["oaec"])
            .pick_file()
        {
            let result = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<Project>(&b).map_err(|e| e.to_string()));
            match result {
                Ok(mut p) if p.version == 1 => {
                    for cloud in &mut p.clouds {
                        if cloud.path.is_relative() {
                            cloud.path = path.parent().unwrap().join(&cloud.path);
                        }
                        if !cloud.path.is_file() || !cloud.transform.is_finite() {
                            self.error =
                                Some("Ongeldig of ontbrekend puntenwolkbestand in project".into());
                            return;
                        }
                    }
                    for model in &mut p.models {
                        if model.is_relative() {
                            *model = path.parent().unwrap().join(&*model);
                        }
                    }
                    if let Some(missing) = p.models.iter().find(|p| !p.is_file()) {
                        self.error = Some(format!("IFC-bestand ontbreekt: {}", missing.display()));
                        return;
                    }
                    if !valid_view(&p.view)
                        || p.views.iter().any(|v| !valid_view(v))
                        || p.rules.iter().any(|r| !r.valid())
                    {
                        self.error =
                            Some("Ongeldige camera- of doorsnedegegevens in project".into());
                        return;
                    }
                    self.clear();
                    self.views = p.views;
                    self.rules = p.rules;
                    self.topics = p.topics;
                    self.results = p.findings;
                    self.results_checked = !self.results.is_empty();
                    self.pending_view = Some(p.view);
                    self.load(p.models, ctx.clone());
                    self.load_clouds(p.clouds, ctx);
                }
                Ok(_) => self.error = Some("Onbekende projectversie".into()),
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn check_clashes(&mut self, ctx: &egui::Context) {
        let hidden = self.expand(&self.hidden);
        let meshes: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .flat_map(|(mi, m)| m.meshes.iter().map(move |mesh| ((mi, mesh.id), mesh)))
            .filter(|(key, _)| !hidden.contains(key))
            .map(|(key, m)| (key, m.clone()))
            .collect();
        let (tx, rx) = mpsc::channel();
        self.clash_rx = Some(rx);
        let ctx = ctx.clone();
        let rules = if self.rules.is_empty() {
            vec![crate::clash::Rule {
                name: "Zichtbare objecten".into(),
                tolerance: self.clash_tolerance,
                ..Default::default()
            }]
        } else {
            self.rules.clone()
        };
        let classes: Vec<_> = meshes
            .iter()
            .map(|(key, _)| {
                self.models[key.0]
                    .elements
                    .get(&key.1)
                    .map(|e| e.class.clone())
                    .unwrap_or_default()
            })
            .collect();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut findings = vec![];
                for rule in rules.iter().filter(|r| r.enabled) {
                    let group = |model: Option<usize>, class: &str| {
                        meshes
                            .iter()
                            .enumerate()
                            .filter(|(i, (key, _))| {
                                model.is_none_or(|m| m == key.0)
                                    && (class.is_empty() || classes[*i].eq_ignore_ascii_case(class))
                            })
                            .map(|(i, _)| i as u32)
                            .collect::<Vec<_>>()
                    };
                    let a = group(rule.model_a, &rule.class_a);
                    let b = (rule.two_sided || rule.model_b.is_some() || !rule.class_b.is_empty())
                        .then(|| group(rule.model_b, &rule.class_b));
                    findings.extend(crate::clash::check_rule(
                        meshes.clone(),
                        &a,
                        b.as_deref(),
                        rule,
                    ));
                }
                findings
            }))
            .map_err(|_| "IfcLite-clashcontrole kon niet worden voltooid".into());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn load_clouds(&mut self, states: Vec<crate::pointcloud::LayerState>, ctx: &egui::Context) {
        if states.is_empty() {
            return;
        }
        if self.cloud_rx.is_some() {
            self.error = Some("Wacht tot de huidige scanbewerking gereed is".into());
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.cloud_rx = Some(rx);
        let ctx = ctx.clone();
        self.status = "Puntenwolk lezen…".into();
        std::thread::spawn(move || {
            for state in states {
                let result = crate::pointcloud::Cloud::load(&state.path).map(|mut cloud| {
                    cloud.state = state;
                    (None, cloud)
                });
                if tx.send(result).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
    }
    fn cloud_tools(&mut self, ctx: &egui::Context) {
        if !self.cloud_window {
            return;
        }
        let mut import = false;
        let mut refresh = false;
        let mut analyze = false;
        let mut refine = false;
        let mut hotspots = false;
        let mut report = false;
        let mut fit = false;
        egui::SidePanel::right("scans-workspace").resizable(true).default_width(500.).width_range(350. ..=700.).show(ctx,|ui| {
            ui.horizontal(|ui|{ui.heading("Scans · As-built");if ui.button("Sluiten").clicked(){self.cloud_window=false;}});
            egui::ScrollArea::both().show(ui,|ui| {
            import=ui.add_enabled(self.cloud_rx.is_none(),egui::Button::new("Puntenwolk openen…")).clicked();
            if self.cloud_rx.is_some(){ui.spinner();ui.label("Puntenwolk verwerken…");}
            for (i,c) in self.clouds.iter_mut().enumerate(){ui.horizontal(|ui| {
                ui.checkbox(&mut c.state.visible,"");
                if ui.selectable_label(self.cloud_index==i,c.state.path.file_name().unwrap_or_default().to_string_lossy()).clicked(){self.cloud_index=i;}
                ui.label(format!("{} / {} punten",c.points.len(),c.total));
            });}
            if let Some(cloud)=self.clouds.get_mut(self.cloud_index) {
                if (cloud.points.len() as u64) < cloud.total {ui.label("Weergave en afwijkingscontrole gebruiken een steekproef van maximaal 2 miljoen punten.");}
                fit=ui.button("Puntenwolk in beeld").clicked();
                ui.separator();ui.label("Registratie: drie overeenkomstige punten in meters (bron → IFC). Bronpunten zijn de originele scancoördinaten.");
                egui::Grid::new("registration-points").show(ui,|ui| {
                    for i in 0..3 {ui.label(format!("{}",i+1));for p in [&mut self.registration_source[i],&mut self.registration_target[i]] {ui.horizontal(|ui| {for axis in 0..3 {ui.add(egui::DragValue::new(&mut p[axis]).speed(0.01).max_decimals(4));}});}ui.end_row();}
                });
                if ui.add_enabled(self.cloud_rx.is_none(),egui::Button::new("Registratie toepassen")).clicked(){match crate::pointcloud::align(self.registration_source,self.registration_target){Ok(t)=>{cloud.state.transform=t;cloud.distances.clear();refresh=true;},Err(e)=>self.error=Some(e)}}
                if ui.add_enabled(self.cloud_rx.is_none(),egui::Button::new("Registratie resetten")).clicked(){cloud.state.transform=glam::DMat4::IDENTITY;cloud.distances.clear();refresh=true;}
                ui.separator();
                ui.horizontal(|ui|{ui.label("Afwijkingsgrens");refresh|=ui.add(egui::DragValue::new(&mut self.cloud_threshold).range(0.001..=10.).speed(0.001).suffix(" m")).changed();});
                analyze=ui.add_enabled(self.cloud_rx.is_none() && !self.models.is_empty(),egui::Button::new("Afstand tot IFC berekenen")).clicked();
                refine=ui.add_enabled(self.cloud_rx.is_none() && !self.models.is_empty(),egui::Button::new("Registratie verfijnen (ICP)")).clicked();
                ui.small("ICP gebruikt maximaal circa 5000 steekproefpunten en alleen correspondenties binnen de afwijkingsgrens. Eerst globaal uitlijnen.");
                if !cloud.distances.is_empty(){let mean=cloud.distances.iter().sum::<f64>()/cloud.distances.len() as f64;let over=cloud.distances.iter().filter(|d|**d>self.cloud_threshold).count();ui.label(format!("Gemiddeld {:.1} mm · {} punten boven grens",mean*1000.,over));ui.label("Groen = op oppervlak; rood = afwijkingsgrens of hoger.");hotspots=ui.button("Maak issues van afwijkingsclusters").clicked();report=ui.button("Afwijkingsrapport PDF…").clicked();}
            }
            });
        });
        if import {
            self.command("cloud-import", ctx);
        }
        if refresh {
            self.renderer.lock().unwrap().upload_clouds(
                &self.gl,
                &self.clouds,
                self.cloud_threshold,
            );
        }
        if fit {
            if let Some(cloud) = self.clouds.get(self.cloud_index) {
                let (min, max) = cloud.bounds();
                self.camera
                    .fit(min.as_vec3(), max.as_vec3(), self.viewport_aspect);
                self.fit_pending = false;
                self.clip = false;
            }
        }
        if hotspots || report {
            if let Some(cloud) = self.clouds.get(self.cloud_index) {
                let clusters = cloud.hotspots(self.cloud_threshold);
                if hotspots {
                    for cluster in &clusters {
                        let source = format!(
                            "scan:{}:{:.2}:{:.2}:{:.2}",
                            cloud.state.path.display(),
                            cluster.center.x,
                            cluster.center.y,
                            cluster.center.z
                        );
                        if self.topics.iter().any(|t| t.source == source) {
                            continue;
                        }
                        let camera = Camera {
                            target: cluster.center.as_vec3(),
                            distance: 3.,
                            ..self.camera.clone()
                        };
                        let mut topic = crate::issues::Topic::new(
                            format!("As-built afwijking {:.1} mm", cluster.max * 1000.),
                            vec![],
                            Some(camera),
                        );
                        topic.source = source;
                        topic.category = "AsBuilt".into();
                        topic.description=format!("{} steekproefpunten in ruimtelijk cluster; maximum {:.1} mm, grens {:.1} mm. Scan: {}",cluster.count,cluster.max*1000.,self.cloud_threshold*1000.,cloud.state.path.display());
                        self.topics.push(topic);
                    }
                    self.topics_open = true;
                }
                if report {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("as-built-afwijkingen.pdf")
                        .save_file()
                    {
                        let mut lines = vec![
                            format!("Datum: {}", chrono::Utc::now().to_rfc3339()),
                            format!(
                                "Scan: {}",
                                cloud
                                    .state
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            ),
                            format!(
                                "Gecontroleerd: {} van {} punten",
                                cloud.points.len(),
                                cloud.total
                            ),
                            format!("Afwijkingsgrens: {:.1} mm", self.cloud_threshold * 1000.),
                            "Afstand tot dichtstbijzijnde IFC-driehoek, zonder teken.".into(),
                            format!(
                                "{} clusters; aangrenzende cellen van 0.5 meter",
                                clusters.len()
                            ),
                            String::new(),
                        ];
                        for (i, c) in clusters.iter().enumerate() {
                            lines.push(format!(
                                "{}: {} punten, max {:.1} mm",
                                i + 1,
                                c.count,
                                c.max * 1000.
                            ));
                            lines.push(format!(
                                "   X {:.3}   Y {:.3}   Z {:.3}",
                                c.center.x, c.center.y, c.center.z
                            ));
                        }
                        match std::fs::File::create(path)
                            .map_err(|e| e.to_string())
                            .and_then(|file| {
                                crate::report::pdf(file, "As-built afwijkingsrapport", &lines)
                            }) {
                            Ok(()) => self.status = "PDF-rapport opgeslagen".into(),
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
            }
        }
        if analyze || refine {
            if let Some(mut cloud) = self.clouds.get(self.cloud_index).cloned() {
                let hidden = self.expand(&self.hidden);
                let meshes: Vec<_> = self
                    .models
                    .iter()
                    .enumerate()
                    .flat_map(|(mi, m)| {
                        let hidden = &hidden;
                        m.meshes
                            .iter()
                            .filter(move |mesh| !hidden.contains(&(mi, mesh.id)))
                            .cloned()
                    })
                    .collect();
                let index = self.cloud_index;
                let threshold = self.cloud_threshold;
                let (tx, rx) = mpsc::channel();
                self.cloud_rx = Some(rx);
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let result = crate::pointcloud::Surface::from_meshes(meshes.into_iter())
                        .and_then(|surface| {
                            if refine {
                                surface.refine(&mut cloud, threshold)?;
                            }
                            cloud.distances = surface.deviations(&cloud);
                            Ok((Some(index), cloud))
                        });
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
            }
        }
    }
    fn ids_editor(&mut self, ctx: &egui::Context) {
        let mut run = false;
        egui::Window::new("IDS-specificatie").open(&mut self.ids_editor_open).default_size([800.,570.]).show(ctx,|ui| {
            ui.horizontal(|ui|{
                if ui.button("Openen…").clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("IDS", &["ids","xml"]).pick_file(){match std::fs::read_to_string(path){Ok(s)=>self.ids_source=s,Err(e)=>self.error=Some(e.to_string())}}}
                if ui.button("Opslaan…").clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("IDS", &["ids"]).set_file_name("specificatie.ids").save_file(){if let Err(e)=std::fs::write(path,&self.ids_source){self.error=Some(e.to_string());}}}
                run=ui.add_enabled(!self.models.is_empty(),egui::Button::new("Valideren")).clicked();
            });
            ui.label("XML-editor. De evaluator ondersteunt een deel van IDS; niet-ondersteunde eisen worden expliciet gemeld.");
            egui::ScrollArea::both().show(ui,|ui|{ui.add(egui::TextEdit::multiline(&mut self.ids_source).code_editor().desired_width(f32::INFINITY).desired_rows(28));});
        });
        if run {
            self.ids(Some(&self.ids_source.clone()));
        }
    }
    fn issue_editor(&mut self, ctx: &egui::Context) {
        if !self.topics_open {
            return;
        }
        let mut view = None;
        let mut snapshot = None;
        egui::TopBottomPanel::bottom("issues-workspace").resizable(true).default_height(330.).height_range(180. ..=650.).show(ctx, |ui| {
            ui.horizontal(|ui| {ui.heading("Issues");if ui.button("Sluiten").clicked(){self.topics_open=false;}});
            egui::ScrollArea::vertical().id_salt("issue-editor-scroll").show(ui,|ui| {
            ui.horizontal(|ui| {ui.checkbox(&mut self.bcf_version3,"BCF 3.0 export");ui.label(format!("{} issues",self.topics.len()));ui.add(egui::TextEdit::singleline(&mut self.topic_filter).hint_text("Zoek titel, status of eigenaar"));});
            ui.separator();
            ui.columns(2, |columns| {
                egui::ScrollArea::vertical().id_salt("topics-list").max_height(480.).show(&mut columns[0], |ui| {
                    let query=self.topic_filter.to_lowercase();
                    for (i,topic) in self.topics.iter().enumerate() {
                        if !format!("{} {} {}",topic.title,topic.status,topic.assignee).to_lowercase().contains(&query) {continue;}
                        if ui.selectable_label(self.active_topic==Some(i),format!("{} · {}",topic.status,topic.title)).clicked() {self.active_topic=Some(i);self.comment_draft.clear();}
                    }
                });
                let ui=&mut columns[1];
                if let Some(topic)=self.active_topic.and_then(|i|self.topics.get_mut(i)) {
                    let before=(topic.title.clone(),topic.description.clone(),topic.status.clone(),topic.priority.clone(),topic.category.clone(),topic.assignee.clone());
                    ui.label("Titel");ui.text_edit_singleline(&mut topic.title);
                    ui.label("Omschrijving");ui.add(egui::TextEdit::multiline(&mut topic.description).desired_rows(3));
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("issue-status").selected_text(&topic.status).show_ui(ui,|ui| {for status in ["Open","InProgress","Resolved","Closed"] {ui.selectable_value(&mut topic.status,status.into(),status);}});
                        egui::ComboBox::from_id_salt("issue-priority").selected_text(&topic.priority).show_ui(ui,|ui| {for priority in ["Low","Normal","High","Critical"] {ui.selectable_value(&mut topic.priority,priority.into(),priority);}});
                    });
                    ui.horizontal(|ui|{ui.label("Eigenaar");ui.text_edit_singleline(&mut topic.assignee);});
                    ui.horizontal(|ui|{ui.label("Categorie");ui.text_edit_singleline(&mut topic.category);});
                    if ui.button("Toon gekoppelde objecten en standpunt").clicked() {view=Some(topic.clone());}
                    if ui.button("Actueel standpunt + snapshot vastleggen").clicked() {snapshot=Some(topic.id.clone());}
                    if !topic.snapshot.is_empty() {ui.label("Snapshot opgeslagen");}
                    ui.separator();ui.label("Opmerkingen");
                    egui::ScrollArea::vertical().id_salt("topic-comments").max_height(120.).show(ui,|ui| {for c in &topic.comments {ui.label(format!("{} · {}",c.author,c.date));ui.label(&c.text);ui.separator();}});
                    ui.text_edit_singleline(&mut self.comment_draft);
                    if ui.add_enabled(!self.comment_draft.trim().is_empty(),egui::Button::new("Opmerking toevoegen")).clicked() {topic.comment("Lokale gebruiker".into(),std::mem::take(&mut self.comment_draft));}
                    if (topic.title.clone(),topic.description.clone(),topic.status.clone(),topic.priority.clone(),topic.category.clone(),topic.assignee.clone())!=before {topic.touch();}
                } else {ui.label("Selecteer een issue of maak een issue vanuit de selectie of controleresultaten.");}
            });
            });
        });
        if let Some(id) = snapshot {
            let hidden = crate::issues::guids(&self.models, self.expand(&self.hidden).into_iter());
            if let Some(topic) = self.topics.iter_mut().find(|t| t.id == id) {
                topic.camera = Some(self.camera.clone());
                topic.hidden = hidden;
                topic.default_visibility = true;
                topic.clipping.clear();
                if self.clip {
                    for axis in 0..3 {
                        for (location, sign) in [(self.clip_min, 1.), (self.clip_max, -1.)] {
                            let mut direction = Vec3::ZERO;
                            direction[axis] = sign;
                            topic.clipping.push(crate::issues::ClipPlane {
                                location,
                                direction,
                            });
                        }
                    }
                }
                topic.touch();
            }
            self.topics_open = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new((
                id,
                self.viewport_rect,
                ctx.pixels_per_point(),
            ))));
        }
        if let Some(topic) = view {
            self.hidden = self
                .models
                .iter()
                .enumerate()
                .flat_map(|(mi, m)| {
                    m.elements
                        .values()
                        .filter(|e| topic.default_visibility == topic.hidden.contains(&e.guid))
                        .map(move |e| (mi, e.id))
                })
                .collect();
            self.clip = false;
            if !topic.clipping.is_empty() {
                self.clip_min = self.bounds.0;
                self.clip_max = self.bounds.1;
                for plane in &topic.clipping {
                    for axis in 0..3 {
                        if plane.direction[axis].abs() > 0.999 {
                            self.clip = true;
                            if plane.direction[axis] > 0. {
                                self.clip_min[axis] = plane.location[axis];
                            } else {
                                self.clip_max[axis] = plane.location[axis];
                            }
                        }
                    }
                }
            }
            self.selected = topic.keys(&self.models).into_iter().collect();
            for key in self.expand(&self.selected) {
                self.hidden.remove(&key);
            }
            if let Some(camera) = topic.camera {
                self.camera = camera;
                self.fit_pending = false;
            } else {
                self.fit(true);
            }
        }
    }
    fn rule_editor(&mut self, ctx: &egui::Context) {
        let mut run = false;
        egui::Window::new("Clashregels").open(&mut self.rules_open).default_width(640.).show(ctx, |ui| {
            ui.label("Kies groepen A en B. Zonder groep B wordt groep A onderling gecontroleerd. Alleen zichtbare objecten worden meegenomen.");
            let mut remove=None;
            egui::ScrollArea::vertical().max_height(480.).show(ui, |ui| {
                for (index,rule) in self.rules.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.group(|ui| {
                            ui.horizontal(|ui| { ui.checkbox(&mut rule.enabled, "Actief"); ui.text_edit_singleline(&mut rule.name); if ui.button("Verwijder").clicked() {remove=Some(index);} });
                            if ui.checkbox(&mut rule.two_sided,"Vergelijk groep A met groep B").changed() && !rule.two_sided {rule.model_b=None;rule.class_b.clear();}
                            for (side,model,class) in [("A",&mut rule.model_a,&mut rule.class_a),("B",&mut rule.model_b,&mut rule.class_b)] {
                                if side=="B" && !rule.two_sided {continue;}
                                ui.horizontal(|ui| {
                                    ui.label(format!("Groep {side}"));
                                    egui::ComboBox::from_id_salt(side).selected_text(model.and_then(|i|self.models.get(i)).map(|m|m.name.as_str()).unwrap_or("Alle modellen")).show_ui(ui, |ui| {
                                        ui.selectable_value(model,None,"Alle modellen");
                                        for (i,m) in self.models.iter().enumerate() {ui.selectable_value(model,Some(i),&m.name);}
                                    });
                                    ui.add(egui::TextEdit::singleline(class).hint_text("IFC-type, leeg = alle").desired_width(170.));
                                });
                            }
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut rule.clearance,"Vrije ruimte");
                                ui.label("Tolerantie"); ui.add(egui::DragValue::new(&mut rule.tolerance).range(0.0..=10.).speed(0.001).suffix(" m"));
                                if rule.clearance {ui.label("Minimumafstand");ui.add(egui::DragValue::new(&mut rule.distance).range(0.0..=100.).speed(0.01).suffix(" m"));}
                            });
                        });
                    });
                }
            });
            if let Some(i)=remove {self.rules.remove(i);}
            ui.horizontal(|ui| {
                if ui.button("Regel toevoegen").clicked() {self.rules.push(Default::default());}
                run=ui.add_enabled(self.clash_rx.is_none() && !self.models.is_empty(),egui::Button::new("Controle uitvoeren")).clicked();
            });
        });
        if run {
            self.check_clashes(ctx);
        }
    }
    fn export_bcf(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("BCF", &["bcfzip"])
            .set_file_name("modelinspectie.bcfzip")
            .save_file()
        {
            let result = std::fs::File::create(path)
                .map_err(|e| e.to_string())
                .and_then(|file| {
                    if !self.topics.is_empty() {
                        return crate::bcf_topics::export(
                            file,
                            &self.topics,
                            self.bcf_version3,
                            self.viewport_aspect,
                        );
                    }
                    crate::bcf::export(
                        file,
                        &self.models,
                        &self.results,
                        &self.camera,
                        &self.expand(&self.selected),
                        &self.expand(&self.hidden),
                    )
                });
            match result {
                Ok(()) => self.status = "BCF geëxporteerd".into(),
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn export_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON-rapport", &["json"])
            .set_file_name("modelcontrole.json")
            .save_file()
        {
            let report = serde_json::json!({"models":self.models.iter().map(|m|m.name.clone()).collect::<Vec<_>>(),"findings":self.results});
            match serde_json::to_vec_pretty(&report)
                .map_err(|e| e.to_string())
                .and_then(|b| std::fs::write(path, b).map_err(|e| e.to_string()))
            {
                Ok(()) => self.status = "Controlerapport geëxporteerd".into(),
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn ids(&mut self, xml: Option<&str>) {
        let source = match xml {
            Some(s) => Ok(s.to_owned()),
            None => match rfd::FileDialog::new()
                .add_filter("IDS", &["ids", "xml"])
                .pick_file()
            {
                Some(p) => std::fs::read_to_string(p).map_err(|e| e.to_string()),
                None => return,
            },
        };
        match source.and_then(|s| validation::check_ids(&self.models, &s)) {
            Ok(r) => {
                self.results = r;
                self.results_checked = true;
                self.results_open = true;
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn command(&mut self, command: &str, ctx: &egui::Context) {
        if matches!(
            command,
            "issues" | "new-issue" | "findings-to-issues" | "bcf-import"
        ) {
            self.tab = 4;
            self.cloud_window = false;
        }
        if matches!(command, "cloud-tools" | "cloud-import") {
            self.tab = 6;
            self.topics_open = false;
        }
        match command {
            "export-glb" | "export-svg" => {
                let hidden = self.expand(&self.hidden);
                let result = if command == "export-glb" {
                    crate::export::glb(&self.models, &hidden)
                } else {
                    crate::export::section_svg(&self.models, &hidden, self.export_height)
                        .map(String::into_bytes)
                };
                match result {
                    Ok(bytes) => {
                        let ext = if command == "export-glb" {
                            "glb"
                        } else {
                            "svg"
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(ext, &[ext])
                            .set_file_name(format!("model.{ext}"))
                            .save_file()
                        {
                            match std::fs::write(path, bytes) {
                                Ok(()) => self.status = format!("{ext}-export voltooid"),
                                Err(e) => self.error = Some(e.to_string()),
                            }
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            "data-table" => {
                self.tab = 5;
                self.data_mode = 0;
            }
            "data-diff" => {
                self.tab = 5;
                self.data_mode = 1;
            }
            "cloud-tools" => {
                self.tab = 6;
                self.cloud_window = true;
            }
            "import" => self.import_dialog(ctx),
            "open" => self.open_project(ctx),
            "save" => self.save_project(),
            "close" => self.clear(),
            "fit" => self.fit(false),
            "focus" => self.fit(true),
            "orbit" => {
                self.walk = false;
                self.camera.yaw = -0.8;
                self.camera.pitch = 0.55;
            }
            "top" => {
                self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                self.camera.pitch = 1.56;
                self.camera.perspective = false;
            }
            "front" => {
                self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                self.camera.pitch = 0.;
                self.camera.perspective = false;
            }
            "solid" => self.prefs.mode = 3,
            "wire" => self.prefs.mode = 1,
            "xray" => self.prefs.mode = 2,
            "isolate" => self.isolate(),
            "hide" => {
                self.hidden.extend(&self.selected);
                self.selected.clear();
            }
            "show" => self.hidden.clear(),
            "slice" => {
                self.clip = !self.clip;
                if self.clip {
                    self.clip_min = self.bounds.0;
                    self.clip_max = self.bounds.1;
                    self.clip_max.z = (self.bounds.0.z + self.bounds.1.z) * 0.5;
                }
                self.inspector = 1;
                self.right_open = true;
            }
            "clip" => {
                self.clip = !self.clip;
                self.inspector = 1;
                self.right_open = true;
            }
            "settings" => {
                self.inspector = 1;
                self.right_open = true;
            }
            "properties" => {
                self.inspector = 0;
                self.right_open = true;
            }
            "cloud-import" => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter(
                        "Puntenwolken",
                        &["las", "laz", "ply", "pcd", "xyz", "pts", "ptx", "e57"],
                    )
                    .pick_files()
                {
                    self.load_clouds(
                        paths
                            .into_iter()
                            .map(|path| crate::pointcloud::LayerState {
                                path,
                                transform: glam::DMat4::IDENTITY,
                                visible: true,
                            })
                            .collect(),
                        ctx,
                    );
                }
            }
            "walk" => {
                self.walk = !self.walk;
                self.camera.perspective = true;
            }
            "measure" => {
                self.measurement = !self.measurement;
                self.area_measurement = false;
                self.points.clear();
            }
            "area" => {
                self.measurement = true;
                self.area_measurement = true;
                self.points.clear();
            }
            "visibility" => {
                if self.points.len() == 2 {
                    let hidden = self.expand(&self.hidden);
                    let meshes = self
                        .models
                        .iter()
                        .enumerate()
                        .flat_map(|(mi, m)| m.meshes.iter().map(move |mesh| ((mi, mesh.id), mesh)))
                        .filter(|(key, _)| !hidden.contains(key))
                        .map(|(key, mesh)| (key, mesh.clone()));
                    self.results =
                        crate::inspection::visibility(meshes, self.points[0], self.points[1]);
                    self.results_checked = true;
                    self.results_open = true;
                    self.status = if self.results.is_empty() {
                        "Zichtlijn vrij".into()
                    } else {
                        format!("{} objecten blokkeren de zichtlijn", self.results.len())
                    };
                } else {
                    self.error = Some("Kies eerst twee punten met Afstand: oogpunt en doel".into());
                }
            }
            "clear-measure" => self.points.clear(),
            "views" => {
                self.inspector = 2;
                self.right_open = true;
            }
            "bookmark" => {
                self.views
                    .push(self.capture(format!("Standpunt {}", self.views.len() + 1)));
                self.inspector = 2;
                self.right_open = true;
            }
            "quality-options" => self.quality_open = true,
            "quality" => {
                let report = crate::quality::run(&self.models, &self.prefs.quality);
                self.status = report
                    .score()
                    .map(|s| {
                        format!(
                            "OpenAEC kwaliteitsscore {s:.1}/100 · {} van {} controles geslaagd",
                            report.passed, report.checked
                        )
                    })
                    .unwrap_or_else(|| "Geen objecten met geometrie om te beoordelen".into());
                self.results = report.findings;
                self.results_checked = true;
                self.results_open = true;
            }
            "clash" => self.check_clashes(ctx),
            "clash-rules" => self.rules_open = true,
            "ids" => self.ids(None),
            "ids-editor" => self.ids_editor_open = true,
            "ils" => self.ids(Some(include_str!(
                "../../ids-bestanden/NL_BIM_Basis_ILS_v2.ids"
            ))),
            "rvb" => self.ids(Some(include_str!(
                "../../ids-bestanden/RVB_BIM_Norm_v1.1.ids"
            ))),
            "results" => self.results_open = !self.results_open,
            "issues" => self.topics_open = true,
            "new-issue" => {
                let components =
                    crate::issues::guids(&self.models, self.expand(&self.selected).into_iter());
                self.topics.push(crate::issues::Topic::new(
                    "Nieuw issue".into(),
                    components,
                    Some(self.camera.clone()),
                ));
                self.active_topic = Some(self.topics.len() - 1);
                self.topics_open = true;
            }
            "findings-to-issues" => {
                let added = crate::issues::reconcile(
                    &mut self.topics,
                    &self.results,
                    &self.models,
                    &self.camera,
                );
                self.status = format!("{added} nieuwe issues; bestaande triage behouden");
                self.topics_open = true;
            }
            "bcf-import" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("BCF", &["bcfzip", "bcf"])
                    .pick_file()
                {
                    match std::fs::File::open(path)
                        .map_err(|e| e.to_string())
                        .and_then(crate::bcf_topics::import)
                    {
                        Ok(topics) => {
                            let mut added = 0;
                            for topic in topics {
                                if !self.topics.iter().any(|t| t.id == topic.id) {
                                    self.topics.push(topic);
                                    added += 1;
                                }
                            }
                            self.status =
                                format!("{added} issues geïmporteerd; bestaande topics behouden");
                            self.topics_open = true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            "bcf" => self.export_bcf(),
            "json" => self.export_results(),
            _ => {}
        }
    }
    fn chrome(&mut self, ctx: &egui::Context) {
        use crate::widgets::{self as w, Icon::*};
        egui::TopBottomPanel::top("studio-tabs")
            .exact_height(34.)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.brand.id(), egui::vec2(24., 29.)));
                    ui.label(RichText::new("OpenAEC").strong());
                    ui.separator();
                    ui.menu_button("Bestand", |ui| {
                        for (label, action) in [
                            ("IFC openen…", "import"),
                            ("Project openen…", "open"),
                            ("Puntenwolk openen…", "cloud-import"),
                            ("Project opslaan…", "save"),
                            ("Project sluiten", "close"),
                        ] {
                            if ui.button(label).clicked() {
                                ui.close_menu();
                                self.command(action, ctx);
                            }
                        }
                        ui.separator();
                        for (label, action) in [
                            ("BCF exporteren…", "bcf"),
                            ("Rapport exporteren…", "json"),
                            ("3D-model exporteren (GLB)…", "export-glb"),
                            ("Horizontale snede (SVG)…", "export-svg"),
                        ] {
                            if ui.button(label).clicked() {
                                ui.close_menu();
                                self.command(action, ctx);
                            }
                        }
                    });
                    for (i, label) in [
                        (1, "Model"),
                        (2, "Beeld"),
                        (5, "Data"),
                        (3, "Controleren"),
                        (4, "Issues"),
                        (6, "Scans"),
                    ] {
                        if w::tab(ui, label, self.tab == i && !self.file_menu).clicked() {
                            self.tab = i;
                            self.topics_open = i == 4;
                            self.cloud_window = i == 6;
                            self.file_menu = false;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Help").clicked() {
                            self.help = true;
                        }
                        if w::icon_button(ui, Sun, "Wissel licht / donker").clicked() {
                            self.prefs.dark = !self.prefs.dark;
                            theme::apply(ctx, self.prefs.dark);
                        }
                        if w::icon_button(ui, Settings, "Beeldinstellingen").clicked() {
                            self.inspector = 1;
                            self.right_open = true;
                        }
                    });
                });
            });
        let mut command = None;
        egui::TopBottomPanel::top("studio-ribbon")
            .exact_height(90.)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                // At narrow desktop widths the command strip scrolls, keeping labels legible.
                egui::ScrollArea::horizontal()
                    .id_salt("ribbon-scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let groups: Vec<RibbonGroup<'_>> = if self.file_menu {
                                vec![
                                    (
                                        "MODEL",
                                        vec![
                                            ("IFC openen", Folder, "import", false),
                                            ("Puntenwolk", Folder, "cloud-import", false),
                                            ("Scans", Layers, "cloud-tools", false),
                                            ("Project", Folder, "open", false),
                                        ],
                                    ),
                                    (
                                        "PROJECT",
                                        vec![
                                            ("Opslaan", Save, "save", false),
                                            ("Sluiten", Close, "close", false),
                                        ],
                                    ),
                                    (
                                        "EXPORTEREN",
                                        vec![
                                            ("BCF", Bookmark, "bcf", false),
                                            ("Rapport", List, "json", false),
                                        ],
                                    ),
                                ]
                            } else {
                                match self.tab {
                                    1 => vec![
                                        (
                                            "SELECTIE",
                                            vec![
                                                ("Passend", Fit, "fit", false),
                                                ("Zoom selectie", Fit, "focus", false),
                                                ("Object", List, "properties", false),
                                            ],
                                        ),
                                        (
                                            "ZICHTBAARHEID",
                                            vec![
                                                ("Isoleer", Isolate, "isolate", false),
                                                ("Verberg", Hide, "hide", false),
                                                ("Alles tonen", Eye, "show", false),
                                            ],
                                        ),
                                        (
                                            "METEN",
                                            vec![
                                                (
                                                    "Afstand",
                                                    Measure,
                                                    "measure",
                                                    self.measurement && !self.area_measurement,
                                                ),
                                                (
                                                    "Oppervlak",
                                                    Measure,
                                                    "area",
                                                    self.area_measurement,
                                                ),
                                                ("Wis meting", Close, "clear-measure", false),
                                            ],
                                        ),
                                    ],
                                    2 => vec![
                                        (
                                            "NAVIGATIE",
                                            vec![
                                                ("Orbit", Orbit, "orbit", false),
                                                ("Lopen", Orbit, "walk", self.walk),
                                                ("Passend", Fit, "fit", false),
                                            ],
                                        ),
                                        (
                                            "WEERGAVE",
                                            vec![
                                                ("Gearceerd", Cube, "solid", self.prefs.mode == 3),
                                                ("Draadmodel", Wire, "wire", self.prefs.mode == 1),
                                                (
                                                    "Transparant",
                                                    Transparent,
                                                    "xray",
                                                    self.prefs.mode == 2,
                                                ),
                                            ],
                                        ),
                                        (
                                            "DOORSNEDE",
                                            vec![
                                                ("Snijvlak", Slice, "slice", self.clip),
                                                ("Snijbox", SectionBox, "clip", false),
                                            ],
                                        ),
                                        (
                                            "ZICHTBAARHEID",
                                            vec![
                                                ("Isoleer", Isolate, "isolate", false),
                                                ("Verberg", Hide, "hide", false),
                                                ("Alles tonen", Eye, "show", false),
                                            ],
                                        ),
                                        (
                                            "METEN",
                                            vec![
                                                ("Afstand", Measure, "measure", self.measurement),
                                                ("Wis meting", Close, "clear-measure", false),
                                            ],
                                        ),
                                    ],
                                    3 => vec![
                                        (
                                            "MODEL CONTROLE",
                                            vec![
                                                ("Modelkwaliteit", Check, "quality", false),
                                                (
                                                    "Kwaliteitsregels",
                                                    Settings,
                                                    "quality-options",
                                                    false,
                                                ),
                                                ("Clashes", Clash, "clash", false),
                                                ("Clashregels", Settings, "clash-rules", false),
                                                ("Zichtlijn", Eye, "visibility", false),
                                            ],
                                        ),
                                        (
                                            "INFORMATIE",
                                            vec![
                                                ("IDS openen", Folder, "ids", false),
                                                ("IDS editor", List, "ids-editor", false),
                                                ("Basis ILS", List, "ils", false),
                                                ("RVB Norm", List, "rvb", false),
                                            ],
                                        ),
                                        (
                                            "RESULTATEN",
                                            vec![
                                                ("Overzicht", List, "results", self.results_open),
                                                ("Exporteer", Save, "json", false),
                                            ],
                                        ),
                                    ],
                                    5 => vec![
                                        (
                                            "TABEL",
                                            vec![
                                                ("Objecten", List, "data-table", false),
                                                ("Versies", Layers, "data-diff", false),
                                            ],
                                        ),
                                        (
                                            "SELECTIE",
                                            vec![
                                                ("Zoom selectie", Fit, "focus", false),
                                                ("Isoleer", Isolate, "isolate", false),
                                                ("Alles tonen", Eye, "show", false),
                                            ],
                                        ),
                                    ],
                                    6 => vec![
                                        (
                                            "SCANS",
                                            vec![
                                                ("Openen", Folder, "cloud-import", false),
                                                (
                                                    "Scanbeheer",
                                                    Settings,
                                                    "cloud-tools",
                                                    self.cloud_window,
                                                ),
                                            ],
                                        ),
                                        (
                                            "BEELD",
                                            vec![
                                                ("Passend", Fit, "fit", false),
                                                ("Snijbox", SectionBox, "clip", false),
                                            ],
                                        ),
                                    ],
                                    _ => vec![
                                        (
                                            "ISSUES",
                                            vec![
                                                ("Issuebeheer", List, "issues", self.topics_open),
                                                ("Nieuw issue", Bookmark, "new-issue", false),
                                                (
                                                    "Uit resultaten",
                                                    Check,
                                                    "findings-to-issues",
                                                    false,
                                                ),
                                                ("BCF import", Folder, "bcf-import", false),
                                                ("BCF export", Save, "bcf", false),
                                            ],
                                        ),
                                        (
                                            "STANDPUNTEN",
                                            vec![
                                                ("Bewaren", Bookmark, "bookmark", false),
                                                ("Beheer", Layers, "views", false),
                                            ],
                                        ),
                                        (
                                            "OBJECT",
                                            vec![
                                                ("Zoom selectie", Fit, "focus", false),
                                                ("Isoleer", Isolate, "isolate", false),
                                            ],
                                        ),
                                    ],
                                }
                            };
                            for (label, tools) in groups {
                                w::group(ui, label, |ui| {
                                    for (text, icon, cmd, active) in tools {
                                        let enabled = match cmd {
                                            "import" | "open" => self.rx.is_none(),
                                            "bcf-import" | "issues" | "new-issue"
                                            | "cloud-tools" => true,
                                            "cloud-import" => self.cloud_rx.is_none(),
                                            "bcf" => {
                                                !self.topics.is_empty() || !self.models.is_empty()
                                            }
                                            "focus" | "isolate" | "hide" => {
                                                !self.selected.is_empty()
                                            }
                                            "clash" | "quality" | "ids" | "ils" | "rvb" => {
                                                !self.models.is_empty()
                                                    && self.rx.is_none()
                                                    && self.clash_rx.is_none()
                                            }
                                            "clear-measure" => !self.points.is_empty(),
                                            _ => !self.models.is_empty(),
                                        };
                                        ui.add_enabled_ui(enabled, |ui| {
                                            if w::tool(ui, text, icon, active).clicked() {
                                                command = Some(cmd);
                                            }
                                        });
                                    }
                                });
                            }
                        });
                    });
            });
        if let Some(c) = command {
            self.command(c, ctx);
        }
        egui::TopBottomPanel::bottom("studio-status")
            .exact_height(27.)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.faint_bg_color)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let summary = if self.rx.is_some() {
                        ui.spinner();
                        &self.progress
                    } else {
                        &self.status
                    };
                    let width = (ui.available_width() - 400.).max(80.);
                    ui.add_sized(
                        [width, 18.],
                        egui::Label::new(RichText::new(summary).size(10.).weak()).truncate(),
                    )
                    .on_hover_text(summary);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Rust · IfcLite").size(10.).weak());
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} modellen · {} objecten",
                                self.models.len(),
                                self.models.iter().map(|m| m.elements.len()).sum::<usize>()
                            ))
                            .size(10.)
                            .weak(),
                        );
                    });
                });
            });
    }
    fn document_bar(&mut self, ctx: &egui::Context) {
        use crate::widgets::{self as w, Icon::*};
        egui::TopBottomPanel::top("studio-documents")
            .exact_height(36.)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.faint_bg_color)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let plan = self.camera.pitch > 1.5;
                    if w::tab(ui, "Coördinatie · 3D", !plan).clicked() {
                        self.command("orbit", ctx);
                    }
                    if w::tab(ui, "Bovenaanzicht", plan).clicked() {
                        self.command("top", ctx);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::icon_button(ui, List, "Controleresultaten tonen / verbergen")
                            .clicked()
                        {
                            self.results_open = !self.results_open;
                        }
                        if w::icon_button(ui, Settings, "Objectpaneel tonen / verbergen").clicked()
                        {
                            self.right_open = !self.right_open;
                        }
                        if w::icon_button(ui, Layers, "Modellen tonen / verbergen").clicked() {
                            self.left_open = !self.left_open;
                        }
                    });
                });
            });
    }
    fn tree(&mut self, ctx: &egui::Context) {
        use crate::widgets::{self as w, Icon::*};
        egui::SidePanel::left("studio-models-v2")
            .default_width(240.)
            .width_range(195.0..=320.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(10),
            )
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.horizontal(|ui| {
                    ui.strong("Modellen");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::icon_button(ui, Close, "Modelpaneel sluiten").clicked() {
                            self.left_open = false;
                        }
                    });
                });
                ui.add_space(4.);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Zoek naam, IFC-type of GUID…")
                        .desired_width(ui.available_width()),
                );
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("model-grouping")
                        .selected_text(
                            [
                                "Ruimtelijk",
                                "IFC-type",
                                "Materiaal",
                                "Classificatie",
                                "Discipline (type)",
                            ][self.grouping as usize],
                        )
                        .show_ui(ui, |ui| {
                            for (i, label) in [
                                "Ruimtelijk",
                                "IFC-type",
                                "Materiaal",
                                "Classificatie",
                                "Discipline (type)",
                            ]
                            .iter()
                            .enumerate()
                            {
                                ui.selectable_value(&mut self.grouping, i as u8, *label);
                            }
                        });
                });
                let query = self.search.to_lowercase();
                let mut action = None;
                let mut focus = false;
                let footer = (self.models.len() as f32 * 31. + 45.).min(190.);
                egui::ScrollArea::vertical()
                    .id_salt("studio-tree")
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - footer).max(80.))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        for (mi, model) in self.models.iter().enumerate() {
                            egui::CollapsingHeader::new(egui::RichText::new(&model.name).strong())
                                .id_salt(("model-v2", mi))
                                .default_open(true)
                                .show(ui, |ui| {
                                    if self.grouping != 0 || !query.is_empty() {
                                        let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
                                        for e in model.elements.values().filter(|e| {
                                            query.is_empty()
                                                || format!(
                                                    "{} {} {} #{}",
                                                    e.name, e.class, e.guid, e.id
                                                )
                                                .to_lowercase()
                                                .contains(&query)
                                        }) {
                                            let group = match self.grouping {
                                                2 => {
                                                    if e.materials.is_empty() {
                                                        "Geen materiaal".into()
                                                    } else {
                                                        e.materials.join(" / ")
                                                    }
                                                }
                                                3 => {
                                                    if e.classifications.is_empty() {
                                                        "Geen classificatie".into()
                                                    } else {
                                                        e.classifications
                                                            .iter()
                                                            .map(|(system, code)| {
                                                                format!("{system}: {code}")
                                                            })
                                                            .collect::<Vec<_>>()
                                                            .join(" / ")
                                                    }
                                                }
                                                4 => crate::model::discipline(&e.class).into(),
                                                _ => e.class.clone(),
                                            };
                                            groups.entry(group).or_default().push(e);
                                        }
                                        if groups.is_empty() {
                                            ui.label(
                                                RichText::new("Geen objecten gevonden").weak(),
                                            );
                                        }
                                        for (class, elements) in groups {
                                            egui::CollapsingHeader::new(format!(
                                                "{} · {}",
                                                class,
                                                elements.len()
                                            ))
                                            .id_salt((mi, class))
                                            .default_open(!query.is_empty())
                                            .show(
                                                ui,
                                                |ui| {
                                                    for e in elements {
                                                        element_row(
                                                            ui,
                                                            mi,
                                                            e,
                                                            &self.selected,
                                                            &mut self.hidden,
                                                            &mut action,
                                                            &mut focus,
                                                        );
                                                    }
                                                },
                                            );
                                        }
                                    } else {
                                        let mut children: BTreeMap<Option<u32>, Vec<u32>> =
                                            BTreeMap::new();
                                        for e in model.elements.values() {
                                            let parent = e.parent.filter(|p| {
                                                model.elements.contains_key(p) && *p != e.id
                                            });
                                            children.entry(parent).or_default().push(e.id);
                                        }
                                        if let Some(roots) = children.get(&None) {
                                            for id in roots {
                                                tree_node(
                                                    ui,
                                                    mi,
                                                    *id,
                                                    model,
                                                    &children,
                                                    &self.selected,
                                                    &mut self.hidden,
                                                    &mut action,
                                                    &mut focus,
                                                    0,
                                                );
                                            }
                                        }
                                    }
                                });
                        }
                        if self.models.is_empty() {
                            ui.add_space(14.);
                            ui.label(RichText::new("Geen modellen geopend").weak());
                            if ui.button("IFC toevoegen…").clicked() {
                                self.import_dialog(ctx);
                            }
                        }
                    });
                ui.separator();
                ui.label(RichText::new("GEKOPPELDE MODELLEN").size(10.).weak());
                let mut visibility = None;
                egui::ScrollArea::vertical()
                    .id_salt("linked-models")
                    .max_height(160.)
                    .show(ui, |ui| {
                        for (mi, m) in self.models.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(egui::vec2(16., 18.), Sense::hover());
                                w::icon(ui.painter(), r, Cube, ui.visuals().weak_text_color());
                                ui.add_sized(
                                    [ui.available_width() - 34., 25.],
                                    egui::Label::new(&m.name).truncate(),
                                )
                                .on_hover_text(format!(
                                    "{} · {} objecten",
                                    m.schema,
                                    m.elements.len()
                                ));
                                let visible = m
                                    .elements
                                    .keys()
                                    .any(|id| !self.hidden.contains(&(mi, *id)));
                                if w::icon_button(
                                    ui,
                                    if visible { Eye } else { Hide },
                                    "Zichtbaarheid model wisselen",
                                )
                                .clicked()
                                {
                                    visibility = Some((mi, !visible));
                                }
                            });
                        }
                    });
                if let Some((mi, show)) = visibility {
                    for id in self.models[mi].elements.keys() {
                        if show {
                            self.hidden.remove(&(mi, *id));
                        } else {
                            self.hidden.insert((mi, *id));
                        }
                    }
                }
                if let Some(key) = action {
                    self.select(key, ctx.input(|i| i.modifiers.ctrl || i.modifiers.command));
                    if focus {
                        self.fit(true);
                    }
                }
            });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        use crate::widgets::{self as w, Icon::*};
        egui::SidePanel::right("studio-inspector")
            .default_width(294.)
            .width_range(255.0..=400.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(10),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, label) in ["Object", "Beeld", "Standpunten"].iter().enumerate() {
                        if w::tab(ui, label, self.inspector == i).clicked() {
                            self.inspector = i;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("studio-inspector-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        match self.inspector {
                            0 => {
                                if let Some(&(mi, id)) = self.selected.iter().min() {
                                    if let Some(e) =
                                        self.models.get(mi).and_then(|m| m.elements.get(&id))
                                    {
                                        ui.add_space(9.);
                                        ui.horizontal(|ui| {
                                            let (r, _) = ui.allocate_exact_size(
                                                egui::vec2(36., 42.),
                                                Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                r,
                                                4.,
                                                ui.visuals().selection.bg_fill,
                                            );
                                            w::icon(ui.painter(), r.shrink(8.), Cube, theme::AMBER);
                                            ui.vertical(|ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(if e.name.is_empty() {
                                                            &e.class
                                                        } else {
                                                            &e.name
                                                        })
                                                        .strong()
                                                        .size(15.),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(&e.name);
                                                ui.label(RichText::new(&e.class).small().weak());
                                            });
                                        });
                                        ui.add_space(9.);
                                        if self.selected.len() > 1 {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} objecten geselecteerd",
                                                    self.selected.len()
                                                ))
                                                .small()
                                                .weak(),
                                            );
                                        }
                                        w::section(ui, "Identiteit");
                                        property_grid(
                                            ui,
                                            "identity",
                                            &BTreeMap::from([
                                                ("GlobalId".into(), e.guid.clone()),
                                                ("Naam".into(), e.name.clone()),
                                                ("Type".into(), e.class.clone()),
                                                ("Model".into(), self.models[mi].name.clone()),
                                            ]),
                                        );
                                        w::section(ui, "Materiaal & geometrie");
                                        let mut geometry = BTreeMap::from([(
                                            "Materiaal".into(),
                                            if e.materials.is_empty() {
                                                "Niet opgegeven".into()
                                            } else {
                                                e.materials.join(", ")
                                            },
                                        )]);
                                        if let Some(mesh) =
                                            self.models[mi].meshes.iter().find(|m| m.id == id)
                                        {
                                            let size = mesh.max - mesh.min;
                                            geometry.insert(
                                                "Begrenzing X".into(),
                                                format!("{:.3} m", size.x),
                                            );
                                            geometry.insert(
                                                "Begrenzing Y".into(),
                                                format!("{:.3} m", size.y),
                                            );
                                            geometry.insert(
                                                "Begrenzing Z".into(),
                                                format!("{:.3} m", size.z),
                                            );
                                        }
                                        property_grid(ui, "geometry", &geometry);
                                        egui::CollapsingHeader::new("IFC-attributen")
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                property_grid(ui, "attributes", &e.attributes)
                                            });
                                        for (name, props) in &e.properties {
                                            w::section(ui, name);
                                            property_grid(ui, name, props);
                                        }
                                        if !e.classifications.is_empty() {
                                            w::section(ui, "Classificatie");
                                            for (system, code) in &e.classifications {
                                                ui.label(format!("{system}: {code}"));
                                            }
                                        }
                                        ui.add_space(10.);
                                        ui.horizontal(|ui| {
                                            if ui.button("Isoleer selectie").clicked() {
                                                self.isolate();
                                            }
                                            if ui.button("Verberg").clicked() {
                                                self.hidden.extend(&self.selected);
                                                self.selected.clear();
                                            }
                                        });
                                    }
                                } else {
                                    ui.add_space(24.);
                                    ui.heading("Objecteigenschappen");
                                    ui.add_space(7.);
                                    ui.label(
                                        RichText::new(
                                            "Selecteer een object in het model of de modelboom.",
                                        )
                                        .weak(),
                                    );
                                    ui.add_space(12.);
                                    ui.label(
                                        RichText::new("Ctrl + klik voor meerdere objecten")
                                            .small()
                                            .weak(),
                                    );
                                }
                            }
                            1 => self.view_settings(ui),
                            _ => {
                                ui.add_space(10.);
                                ui.heading("Standpunten");
                                ui.label(
                                    RichText::new("Camera, zichtbaarheid en doorsneden")
                                        .small()
                                        .weak(),
                                );
                                ui.add_space(8.);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.view_name)
                                        .hint_text("Naam van standpunt")
                                        .desired_width(ui.available_width()),
                                );
                                if ui
                                    .add_enabled(
                                        !self.models.is_empty(),
                                        egui::Button::new("Huidig standpunt bewaren"),
                                    )
                                    .clicked()
                                {
                                    let name = if self.view_name.trim().is_empty() {
                                        format!("Standpunt {}", self.views.len() + 1)
                                    } else {
                                        self.view_name.trim().into()
                                    };
                                    self.views.push(self.capture(name));
                                    self.view_name.clear();
                                }
                                ui.add_space(12.);
                                let mut restore = None;
                                let mut remove = None;
                                for (i, v) in self.views.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add_sized(
                                                [ui.available_width() - 34., 30.],
                                                egui::Button::new(&v.name).truncate(),
                                            )
                                            .clicked()
                                        {
                                            restore = Some(v.clone());
                                        }
                                        if w::icon_button(ui, Close, "Standpunt verwijderen")
                                            .clicked()
                                        {
                                            remove = Some(i);
                                        }
                                    });
                                }
                                if let Some(v) = restore {
                                    self.restore(v);
                                }
                                if let Some(i) = remove {
                                    self.views.remove(i);
                                }
                                ui.add_space(12.);
                                if ui
                                    .add_enabled(
                                        !self.models.is_empty(),
                                        egui::Button::new("Exporteer BCF-standpunt…"),
                                    )
                                    .clicked()
                                {
                                    self.export_bcf();
                                }
                                ui.label(
                                    RichText::new("Sla het project op om standpunten te bewaren.")
                                        .small()
                                        .weak(),
                                );
                            }
                        }
                    });
            });
    }
    fn view_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Weergave");
        for (i, name) in [
            "Gekleurd op IFC-type",
            "Draadmodel",
            "Transparant",
            "Gearceerd · neutraal",
        ]
        .iter()
        .enumerate()
        {
            ui.radio_value(&mut self.prefs.mode, i as u8, *name);
        }
        ui.checkbox(&mut self.prefs.edges, "Driehoeksranden");
        ui.separator();
        ui.heading("Camera");
        ui.horizontal_wrapped(|ui| {
            for (label, yaw, pitch) in [
                ("3D", -0.8, 0.55),
                ("Boven", -std::f32::consts::FRAC_PI_2, 1.56),
                ("Voor", -std::f32::consts::FRAC_PI_2, 0.),
                ("Rechts", 0., 0.),
            ] {
                if ui.button(label).clicked() {
                    self.camera.yaw = yaw;
                    self.camera.pitch = pitch;
                }
            }
        });
        ui.checkbox(&mut self.camera.perspective, "Perspectief");
        ui.add(egui::Slider::new(&mut self.camera.fov, 20.0..=90.0).text("Beeldhoek"));
        if ui.button("Alles in beeld").clicked() {
            self.fit(false);
        }
        ui.separator();
        ui.heading("Doorsneden");
        ui.checkbox(&mut self.clip, "Doorsnedebox actief");
        ui.add_enabled_ui(self.clip, |ui| {
            for axis in 0..3 {
                ui.label(["X-as", "Y-as", "Z-as"][axis]);
                let lo = self.bounds.0[axis];
                let hi = self.bounds.1[axis].max(lo + 0.001);
                ui.add(egui::Slider::new(&mut self.clip_min[axis], lo..=hi).text("Min"));
                ui.add(egui::Slider::new(&mut self.clip_max[axis], lo..=hi).text("Max"));
                if self.clip_min[axis] > self.clip_max[axis] {
                    self.clip_max[axis] = self.clip_min[axis];
                }
            }
            if ui.button("Herstel doorsnedebox").clicked() {
                self.clip_min = self.bounds.0;
                self.clip_max = self.bounds.1;
            }
        });
        ui.separator();
        ui.heading("Controle");
        ui.add(
            egui::DragValue::new(&mut self.clash_tolerance)
                .range(0.0..=0.1)
                .speed(0.001)
                .suffix(" m clashtolerantie"),
        );
        ui.separator();
        ui.heading("Bediening");
        ui.label("Linker muisknop slepen: draaien\nRechts/midden slepen: verschuiven\nScrollen: zoomen\nDubbelklik: zoom naar object\nHome: alles in beeld\nF: zoom selectie · H: verberg\nI: isoleer · Esc: deselecteer");
    }
    fn result_filters(&mut self, ui: &mut egui::Ui) {
        for (i, label) in ["Alle", "Fouten", "Overige"].iter().enumerate() {
            let count = self
                .results
                .iter()
                .filter(|r| match i {
                    1 => r.severity == "Fout",
                    2 => r.severity != "Fout",
                    _ => true,
                })
                .count();
            ui.selectable_value(&mut self.result_filter, i, format!("{label} ({count})"));
        }
    }
    fn results_panel(&mut self, ctx: &egui::Context) {
        if !self.results_open {
            return;
        }
        egui::TopBottomPanel::bottom("studio-results-v2")
            .default_height(174.)
            .min_height(100.)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(8),
            )
            .show(ctx, |ui| {
                let compact = ui.available_width() < 650.;
                ui.horizontal(|ui| {
                    ui.strong("Controleresultaten");
                    ui.add_space(8.);
                    if !compact {
                        self.result_filters(ui);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::widgets::icon_button(
                            ui,
                            crate::widgets::Icon::Close,
                            "Resultaten sluiten",
                        )
                        .clicked()
                        {
                            self.results_open = false;
                        }
                        ui.menu_button("Exporteren", |ui| {
                            if ui.button("BCF 2.1").clicked() {
                                self.export_bcf();
                                ui.close_menu();
                            }
                            if ui.button("JSON-rapport").clicked() {
                                self.export_results();
                                ui.close_menu();
                            }
                        });
                    });
                });
                if compact {
                    ui.horizontal(|ui| self.result_filters(ui));
                }
                ui.add_space(4.);
                let width = ui.available_width();
                let columns = [width * 0.11, width * 0.39, width * 0.18, width * 0.32];
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.;
                        ui.horizontal(|ui| {
                            for (label, w) in ["ID", "Omschrijving", "Resultaat", "Object"]
                                .iter()
                                .zip(columns)
                            {
                                ui.add_sized(
                                    [w, 24.],
                                    egui::Label::new(RichText::new(*label).size(11.).weak())
                                        .truncate(),
                                );
                            }
                        });
                    });
                let mut action = None;
                egui::ScrollArea::vertical()
                    .id_salt("studio-results-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0., 0.);
                        for (index, r) in
                            self.results.iter().enumerate().filter(|(_, r)| {
                                match self.result_filter {
                                    1 => r.severity == "Fout",
                                    2 => r.severity != "Fout",
                                    _ => true,
                                }
                            })
                        {
                            let key = r.key();
                            let selected = key.is_some_and(|k| self.selected.contains(&k));
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(width, 29.), Sense::click());
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    0.,
                                    if selected {
                                        ui.visuals().selection.bg_fill
                                    } else {
                                        ui.visuals().faint_bg_color
                                    },
                                );
                            }
                            ui.painter().hline(
                                rect.x_range(),
                                rect.bottom(),
                                ui.visuals().widgets.noninteractive.bg_stroke,
                            );
                            let object = key
                                .and_then(|(mi, id)| self.models.get(mi)?.elements.get(&id))
                                .map(|e| {
                                    if e.name.is_empty() {
                                        e.class.clone()
                                    } else {
                                        e.name.clone()
                                    }
                                })
                                .unwrap_or_else(|| format!("Model {}", r.model + 1));
                            let color = match r.severity.as_str() {
                                "Fout" => Color32::from_rgb(210, 63, 48),
                                "Geslaagd" => Color32::from_rgb(40, 150, 80),
                                _ => theme::AMBER,
                            };
                            let mut x = rect.left();
                            for ((text, color), w) in [
                                (format!("{:03}", index + 1), ui.visuals().weak_text_color()),
                                (r.message.clone(), ui.visuals().text_color()),
                                (r.severity.clone(), color),
                                (object, ui.visuals().text_color()),
                            ]
                            .into_iter()
                            .zip(columns)
                            {
                                let cell = egui::Rect::from_min_size(
                                    egui::pos2(x + 6., rect.top()),
                                    egui::vec2((w - 12.).max(1.), rect.height()),
                                );
                                let painter =
                                    ui.painter().with_clip_rect(cell.intersect(ui.clip_rect()));
                                painter.text(
                                    cell.left_center(),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    egui::FontId::proportional(11.),
                                    color,
                                );
                                x += w;
                            }
                            if response
                                .clone()
                                .on_hover_text(format!("{}\n{}", r.rule, r.message))
                                .clicked()
                            {
                                if let Some(key) = key {
                                    action = Some((key, r.related));
                                }
                            }
                        }
                        if self.results.is_empty() {
                            ui.add_space(12.);
                            ui.label(
                                RichText::new(if self.results_checked {
                                    "Geen bevindingen in de uitgevoerde controle."
                                } else {
                                    "Start een controle via het lint Controleren."
                                })
                                .small()
                                .weak(),
                            );
                        }
                    });
                if let Some((key, related)) = action {
                    self.hidden.clear();
                    self.select(key, false);
                    if let Some(other) = related {
                        self.selected.insert(other);
                    }
                    self.inspector = 0;
                    self.right_open = true;
                    self.fit(true);
                }
            });
    }
    fn hit(&self, x: f32, y: f32, aspect: f32) -> Option<(Key, DVec3)> {
        let hidden = self.expand(&self.hidden);
        let (origin, dir) = self.camera.ray(x, y, aspect);
        let mut closest = f32::INFINITY;
        let mut hit = None;
        for (mi, model) in self.models.iter().enumerate() {
            for mesh in &model.meshes {
                let key = (mi, mesh.id);
                let origin =
                    (self.camera.target.as_dvec3() + origin.as_dvec3() - mesh.origin).as_vec3();
                if hidden.contains(&key)
                    || !camera::ray_box(
                        origin,
                        dir,
                        (mesh.min - mesh.origin).as_vec3(),
                        (mesh.max - mesh.origin).as_vec3(),
                    )
                {
                    continue;
                }
                for tri in mesh.indices.as_chunks::<3>().0 {
                    let v = |id: u32| {
                        let p = mesh.vertices[id as usize];
                        Vec3::new(p[0], p[1], p[2])
                    };
                    if let Some(t) =
                        camera::ray_triangle(origin, dir, v(tri[0]), v(tri[1]), v(tri[2]))
                    {
                        if t >= closest {
                            continue;
                        }
                        let p = mesh.origin + (origin + dir * t).as_dvec3();
                        if self.clip
                            && (p.cmplt(self.clip_min.as_dvec3()).any()
                                || p.cmpgt(self.clip_max.as_dvec3()).any())
                        {
                            continue;
                        }
                        closest = t;
                        hit = Some((key, p));
                    }
                }
            }
        }
        hit
    }
    fn viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                self.viewport_rect = rect;
                self.viewport_aspect = rect.aspect_ratio();
                if self.fit_pending {
                    self.fit(false);
                    self.fit_pending = false;
                }
                let over_navigation = ctx
                    .input(|i| i.pointer.hover_pos())
                    .is_some_and(|p| p.x > rect.right() - 195. && p.y < rect.top() + 160.);
                if response.clicked() {
                    response.request_focus();
                }
                if self.walk && response.has_focus() && !ctx.wants_keyboard_input() {
                    let (movement, dt) = ctx.input(|i| {
                        let mut v = Vec3::ZERO;
                        if i.key_down(egui::Key::W) {
                            v.y += 1.;
                        }
                        if i.key_down(egui::Key::S) {
                            v.y -= 1.;
                        }
                        if i.key_down(egui::Key::D) {
                            v.x += 1.;
                        }
                        if i.key_down(egui::Key::A) {
                            v.x -= 1.;
                        }
                        if i.key_down(egui::Key::E) {
                            v.z += 1.;
                        }
                        if i.key_down(egui::Key::Q) {
                            v.z -= 1.;
                        }
                        (v, i.stable_dt.min(0.05))
                    });
                    let forward = -self.camera.direction();
                    let right = Vec3::Z.cross(-forward).normalize();
                    self.camera.target +=
                        (forward * movement.y + right * movement.x + Vec3::Z * movement.z)
                            * dt
                            * 2.;
                    ctx.request_repaint();
                }
                let delta = ctx.input(|i| i.pointer.delta());
                if !over_navigation && response.dragged_by(egui::PointerButton::Primary) {
                    let eye = self.camera.eye();
                    self.camera.orbit(delta.x, delta.y);
                    if self.walk {
                        self.camera.target = eye - self.camera.direction() * self.camera.distance;
                    }
                }
                if response.dragged_by(egui::PointerButton::Secondary)
                    || response.dragged_by(egui::PointerButton::Middle)
                {
                    self.camera.pan(delta.x, delta.y, rect.height());
                }
                if response.hovered() && !over_navigation {
                    self.camera.zoom(ctx.input(|i| i.smooth_scroll_delta.y));
                }
                if !over_navigation && (response.clicked() || response.double_clicked()) {
                    if let Some(p) = response.interact_pointer_pos() {
                        let hit = self.hit(
                            (p.x - rect.left()) / rect.width() * 2. - 1.,
                            1. - (p.y - rect.top()) / rect.height() * 2.,
                            rect.aspect_ratio(),
                        );
                        if let Some((key, point)) = hit {
                            if self.measurement {
                                if !self.area_measurement && self.points.len() == 2 {
                                    self.points.clear();
                                }
                                self.points.push(point);
                            } else {
                                self.select(
                                    key,
                                    ctx.input(|i| i.modifiers.ctrl || i.modifiers.command),
                                );
                                if response.double_clicked() {
                                    self.fit(true);
                                }
                            }
                        } else if !ctx.input(|i| i.modifiers.ctrl) {
                            self.selected.clear();
                        }
                    }
                }
                let d = Draw {
                    hidden_clouds: self
                        .clouds
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| !c.state.visible)
                        .map(|(i, _)| i)
                        .collect(),
                    matrix: self
                        .camera
                        .relative_matrix(rect.aspect_ratio(), self.camera.target),
                    reference: self.camera.target,
                    eye: self.camera.eye(),
                    selected: self.expand(&self.selected),
                    hidden: self.expand(&self.hidden),
                    clip_min: if self.clip {
                        self.clip_min
                    } else {
                        Vec3::splat(-f32::MAX)
                    },
                    clip_max: if self.clip {
                        self.clip_max
                    } else {
                        Vec3::splat(f32::MAX)
                    },
                    mode: self.prefs.mode,
                    edges: self.prefs.edges,
                    background: if self.prefs.dark {
                        [0.165, 0.165, 0.196]
                    } else {
                        [0.91, 0.92, 0.925]
                    },
                };
                let renderer = self.renderer.clone();
                ui.painter().add(egui::PaintCallback {
                    rect,
                    callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                        renderer.lock().unwrap().paint(painter.gl(), &d);
                    })),
                });
                let text_color = if self.prefs.dark {
                    Color32::LIGHT_GRAY
                } else {
                    theme::FORGE
                };
                ui.painter().text(
                    rect.left_top() + egui::vec2(18., 18.),
                    egui::Align2::LEFT_TOP,
                    if self.clip {
                        "Coördinatie · doorsnede actief"
                    } else {
                        "Coördinatie"
                    },
                    egui::FontId::proportional(13.),
                    text_color,
                );
                if let Some(&(mi, id)) = self.selected.iter().min() {
                    if let Some(e) = self.models.get(mi).and_then(|m| m.elements.get(&id)) {
                        let label = format!(
                            "{}  ·  {}  ·  #{}",
                            if e.name.is_empty() { &e.class } else { &e.name },
                            e.class,
                            e.id
                        );
                        let chip = egui::Rect::from_center_size(
                            rect.center_bottom() + egui::vec2(0., -25.),
                            egui::vec2((rect.width() - 150.).clamp(80., 440.), 30.),
                        );
                        ui.painter().rect_filled(chip, 4., ui.visuals().panel_fill);
                        ui.painter().rect_stroke(
                            chip,
                            4.,
                            ui.visuals().widgets.noninteractive.bg_stroke,
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().with_clip_rect(chip.shrink(7.)).text(
                            chip.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(11.),
                            text_color,
                        );
                    }
                }
                if !self.models.is_empty() {
                    let cube_rect = egui::Rect::from_min_size(
                        rect.right_top() + egui::vec2(-195., 12.),
                        egui::vec2(145., 140.),
                    );
                    if let Some(direction) = crate::view_cube::show(ui, cube_rect, &self.camera) {
                        self.camera.yaw = direction.y.atan2(direction.x);
                        self.camera.pitch = direction
                            .z
                            .atan2(direction.x.hypot(direction.y))
                            .clamp(-1.56, 1.56);
                    }
                    for (i, (label, cmd)) in [
                        ("3D", "orbit"),
                        ("+", "zoom-in"),
                        ("−", "zoom-out"),
                        ("↔", "fit"),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let button = egui::Rect::from_min_size(
                            rect.right_top() + egui::vec2(-40., 18. + i as f32 * 35.),
                            egui::vec2(29., 29.),
                        );
                        let tip = match *cmd {
                            "orbit" => "Isometrisch aanzicht",
                            "zoom-in" => "Inzoomen",
                            "zoom-out" => "Uitzoomen",
                            _ => "Alles in beeld",
                        };
                        let nav_response =
                            ui.interact(button, ui.id().with(("navigation", cmd)), Sense::click());
                        ui.painter().rect_filled(
                            button,
                            3.,
                            if nav_response.hovered() {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().panel_fill
                            },
                        );
                        ui.painter().rect_stroke(
                            button,
                            3.,
                            ui.visuals().widgets.noninteractive.bg_stroke,
                            egui::StrokeKind::Inside,
                        );
                        if matches!(*cmd, "orbit" | "fit") {
                            crate::widgets::icon(
                                ui.painter(),
                                button.shrink(6.),
                                if *cmd == "orbit" {
                                    crate::widgets::Icon::Cube
                                } else {
                                    crate::widgets::Icon::Fit
                                },
                                text_color,
                            );
                        } else {
                            ui.painter().text(
                                button.center(),
                                egui::Align2::CENTER_CENTER,
                                *label,
                                egui::FontId::proportional(16.),
                                text_color,
                            );
                        }
                        if nav_response.on_hover_text(tip).clicked() {
                            match *cmd {
                                "zoom-in" => self.camera.zoom(100.),
                                "zoom-out" => self.camera.zoom(-100.),
                                _ => self.command(cmd, ctx),
                            }
                        }
                    }
                }
                if self.models.is_empty() {
                    let center = rect.center();
                    ui.painter().text(
                        center - egui::vec2(0., 36.),
                        egui::Align2::CENTER_CENTER,
                        "Een helder beeld van je model",
                        egui::FontId::new(25., egui::FontFamily::Name("heading".into())),
                        text_color,
                    );
                    ui.painter().text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "Open een IFC-bestand · verken, inspecteer en controleer",
                        egui::FontId::proportional(14.),
                        text_color,
                    );
                    let button = egui::Rect::from_center_size(
                        center + egui::vec2(0., 52.),
                        egui::vec2(150., 36.),
                    );
                    if ui
                        .put(
                            button,
                            egui::Button::new("IFC toevoegen…").fill(theme::AMBER),
                        )
                        .clicked()
                        && self.rx.is_none()
                    {
                        self.import_dialog(ctx);
                    }
                }
                // Orientation axes are a UI overlay, not model geometry.
                let origin = rect.left_bottom() + egui::vec2(42., -65.);
                let dir = self.camera.direction();
                let right = Vec3::Z.cross(dir).normalize();
                let up = dir.cross(right);
                for (axis, label, color) in [
                    (Vec3::X, "X", Color32::LIGHT_RED),
                    (Vec3::Y, "Y", Color32::LIGHT_GREEN),
                    (Vec3::Z, "Z", Color32::LIGHT_BLUE),
                ] {
                    let end = origin + egui::vec2(axis.dot(right), -axis.dot(up)) * 30.;
                    ui.painter()
                        .line_segment([origin, end], egui::Stroke::new(2.0_f32, color));
                    ui.painter().text(
                        end,
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::monospace(12.),
                        color,
                    );
                }
                let project = |p: DVec3| {
                    let p = self
                        .camera
                        .relative_matrix(rect.aspect_ratio(), self.camera.target)
                        .project_point3((p - self.camera.target.as_dvec3()).as_vec3());
                    egui::pos2(
                        rect.center().x + p.x * rect.width() * 0.5,
                        rect.center().y - p.y * rect.height() * 0.5,
                    )
                };
                for point in &self.points {
                    ui.painter()
                        .circle_filled(project(*point), 5., theme::AMBER);
                }
                if self.area_measurement && self.points.len() > 1 {
                    let points: Vec<_> = self.points.iter().map(|p| project(*p)).collect();
                    for pair in points.windows(2) {
                        ui.painter().line_segment(
                            [pair[0], pair[1]],
                            egui::Stroke::new(2_f32, theme::AMBER),
                        );
                    }
                    if points.len() > 2 {
                        ui.painter().line_segment(
                            [points[points.len() - 1], points[0]],
                            egui::Stroke::new(2_f32, theme::AMBER),
                        );
                    }
                    let label = crate::inspection::polygon_area(&self.points)
                        .map(|a| format!("{a:.3} m²"))
                        .unwrap_or_else(|e| e);
                    ui.painter().text(
                        points[0] + egui::vec2(0., -15.),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        egui::FontId::proportional(13.),
                        theme::AMBER,
                    );
                } else if self.points.len() == 2 {
                    let a = project(self.points[0]);
                    let b = project(self.points[1]);
                    ui.painter()
                        .line_segment([a, b], egui::Stroke::new(2.0_f32, theme::AMBER));
                    ui.painter().text(
                        a + (b - a) * 0.5,
                        egui::Align2::CENTER_BOTTOM,
                        format!("{:.3} m", self.points[0].distance(self.points[1])),
                        egui::FontId::proportional(16.),
                        theme::AMBER,
                    );
                }
            });
    }
}
fn property_grid(ui: &mut egui::Ui, id: &str, props: &BTreeMap<String, String>) {
    ui.push_id(id, |ui| {
        let width = ui.available_width();
        for (name, value) in props {
            let r = ui
                .horizontal_top(|ui| {
                    ui.add_sized(
                        [width * 0.40, 23.],
                        egui::Label::new(RichText::new(name).size(11.).weak()).truncate(),
                    )
                    .on_hover_text(name);
                    ui.add_sized(
                        [width * 0.60 - 8., 23.],
                        egui::Label::new(
                            RichText::new(if value.is_empty() { "—" } else { value }).size(11.),
                        )
                        .truncate()
                        .selectable(true),
                    )
                    .on_hover_text(value);
                })
                .response
                .rect;
            ui.painter().hline(
                r.x_range(),
                r.bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
        }
        ui.add_space(9.);
    });
}
fn element_row(
    ui: &mut egui::Ui,
    mi: usize,
    e: &crate::model::Element,
    selected: &HashSet<Key>,
    hidden: &mut HashSet<Key>,
    action: &mut Option<Key>,
    focus: &mut bool,
) {
    use crate::widgets::{self as w, Icon};
    let key = (mi, e.id);
    let chosen = selected.contains(&key);
    ui.horizontal(|ui| {
        let width = (ui
            .available_width()
            .min(ui.clip_rect().right() - ui.cursor().left())
            - 32.)
            .max(20.);
        let label = if e.name.is_empty() {
            format!("{} #{}", e.class, e.id)
        } else {
            e.name.clone()
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 27.), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), chosen, &label)
        });
        if chosen || response.hovered() {
            ui.painter().rect_filled(
                rect,
                3.,
                if chosen {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().faint_bg_color
                },
            );
        }
        let color = if chosen {
            theme::AMBER
        } else if hidden.contains(&key) {
            ui.visuals().weak_text_color()
        } else {
            ui.visuals().text_color()
        };
        w::icon(
            ui.painter(),
            egui::Rect::from_min_size(
                rect.left_center() + egui::vec2(3., -7.),
                egui::vec2(14., 14.),
            ),
            if e.class.contains("Storey") || e.class.contains("Slab") {
                Icon::Layers
            } else {
                Icon::Cube
            },
            color,
        );
        let painter = ui.painter().with_clip_rect(
            egui::Rect::from_min_max(rect.min + egui::vec2(23., 0.), rect.max)
                .intersect(ui.clip_rect()),
        );
        painter.text(
            rect.left_center() + egui::vec2(23., 0.),
            egui::Align2::LEFT_CENTER,
            &label,
            egui::FontId::proportional(11.),
            color,
        );
        if response
            .clone()
            .on_hover_text(format!("{label}\n{} · #{}", e.class, e.id))
            .clicked()
            || response.double_clicked()
        {
            *action = Some(key);
            *focus = response.double_clicked();
        }
        if w::icon_button(
            ui,
            if hidden.contains(&key) {
                Icon::Hide
            } else {
                Icon::Eye
            },
            "Object tonen / verbergen",
        )
        .clicked()
            && !hidden.remove(&key)
        {
            hidden.insert(key);
        }
    });
}
#[allow(clippy::too_many_arguments)]
fn tree_node(
    ui: &mut egui::Ui,
    mi: usize,
    id: u32,
    model: &Model,
    children: &BTreeMap<Option<u32>, Vec<u32>>,
    selected: &HashSet<Key>,
    hidden: &mut HashSet<Key>,
    action: &mut Option<Key>,
    focus: &mut bool,
    depth: usize,
) {
    if depth > 40 {
        return;
    }
    let e = &model.elements[&id];
    if let Some(kids) = children.get(&Some(id)) {
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id((mi, id)),
            depth < 4,
        )
        .show_header(ui, |ui| {
            element_row(ui, mi, e, selected, hidden, action, focus);
        })
        .body(|ui| {
            if e.class == "IfcBuildingStorey" {
                let mut groups: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
                for child in kids {
                    groups
                        .entry(&model.elements[child].class)
                        .or_default()
                        .push(*child);
                }
                for (class, ids) in groups {
                    let name = match class {
                        "IfcWall" | "IfcWallStandardCase" => "Wanden",
                        "IfcSlab" => "Vloeren",
                        "IfcBeam" => "Balken",
                        "IfcColumn" => "Kolommen",
                        "IfcWindow" => "Ramen",
                        "IfcDoor" => "Deuren",
                        "IfcRoof" => "Daken",
                        "IfcSpace" => "Ruimten",
                        _ => class,
                    };
                    egui::CollapsingHeader::new(format!("{name} · {}", ids.len()))
                        .id_salt(("storey-class", mi, id, class))
                        .default_open(ids.iter().any(|id| selected.contains(&(mi, *id))))
                        .show(ui, |ui| {
                            for child in ids {
                                tree_node(
                                    ui,
                                    mi,
                                    child,
                                    model,
                                    children,
                                    selected,
                                    hidden,
                                    action,
                                    focus,
                                    depth + 1,
                                );
                            }
                        });
                }
            } else {
                for child in kids {
                    tree_node(
                        ui,
                        mi,
                        *child,
                        model,
                        children,
                        selected,
                        hidden,
                        action,
                        focus,
                        depth + 1,
                    );
                }
            }
        });
    } else {
        element_row(ui, mi, e, selected, hidden, action, focus);
    }
}
fn valid_view(v: &View) -> bool {
    v.camera.target.is_finite()
        && v.camera.distance.is_finite()
        && v.camera.distance > 0.
        && v.camera.yaw.is_finite()
        && v.camera.pitch.is_finite()
        && v.camera.pitch.abs() <= 1.56
        && (20.0..=90.0).contains(&v.camera.fov)
        && v.clip_min.is_finite()
        && v.clip_max.is_finite()
        && v.clip_min.cmple(v.clip_max).all()
}
impl eframe::App for BimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot {
                image, user_data, ..
            } = event
            {
                if let Some((id, rect, scale)) = user_data
                    .data
                    .as_ref()
                    .and_then(|d| d.downcast_ref::<(String, egui::Rect, f32)>())
                {
                    if rect.is_positive() {
                        let image = image.region(rect, Some(*scale));
                        let icon = egui::IconData {
                            rgba: image.pixels.iter().flat_map(|p| p.to_array()).collect(),
                            width: image.size[0] as u32,
                            height: image.size[1] as u32,
                        };
                        match eframe::icon_data::IconDataExt::to_png_bytes(&icon) {
                            Ok(bytes) => {
                                if let Some(topic) = self.topics.iter_mut().find(|t| t.id == *id) {
                                    topic.snapshot = bytes;
                                    topic.touch();
                                }
                                self.topics_open = true;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
            }
        }
        #[cfg(feature = "screenshot")]
        if let Some(path) = std::env::var_os("OPENAEC_SCREENSHOT_TO") {
            ctx.input_mut(|i| i.pointer = Default::default());
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        std::path::Path::new(&path),
                        &pixels,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Screenshot mislukt: {e}");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if self.rx.is_none() && ctx.cumulative_pass_nr() > 5 && !self.screenshot_requested {
                self.screenshot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint();
        }
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| p.extension().is_some_and(|s| s.eq_ignore_ascii_case("ifc")))
                .collect()
        });
        if !dropped.is_empty() && self.rx.is_none() {
            self.load(dropped, ctx.clone());
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O))
                && self.rx.is_none()
            {
                self.import_dialog(ctx);
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S))
                && !self.models.is_empty()
            {
                self.save_project();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                self.fit(false);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                self.fit(true);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::H)) {
                self.hidden.extend(&self.selected);
                self.selected.clear();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::I)) {
                self.isolate();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.selected.clear();
                self.points.clear();
            }
        }
        self.chrome(ctx);
        self.data_panel(ctx);
        self.issue_editor(ctx);
        self.cloud_tools(ctx);
        if self.left_open {
            self.tree(ctx);
        }
        if self.right_open && !self.cloud_window {
            self.inspector(ctx);
        }
        self.document_bar(ctx);
        if self.tab != 5 && self.tab != 6 && !self.topics_open {
            self.results_panel(ctx);
        }
        self.viewport(ctx);
        if let Some(error) = self.error.clone() {
            egui::Window::new("Actie niet voltooid")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("Sluiten").clicked() {
                        self.error = None;
                    }
                });
        }
        egui::Window::new("Over OpenAEC BIM Validator").open(&mut self.help).show(ctx,|ui|{ui.heading("OpenAEC BIM Validator");ui.label("Native Rust IFC-viewer · IfcLite 20.4.0");ui.label("Importeer meerdere IFC-modellen, bekijk eigenschappen en sla standpunten op. Projectbestanden verwijzen naar je lokale IFC-bestanden.");ui.separator();ui.label("Modelkwaliteit controleert namen, GUIDs, proxies en importmeldingen. IDS-ondersteuning is gedeeltelijk; onondersteunde controles krijgen nooit een geslaagd-status.");ui.hyperlink_to("OpenAEC Style Book","https://github.com/OpenAEC-Foundation/OpenAEC-style-book");ui.hyperlink_to("IfcLite","https://github.com/LTplus-AG/ifc-lite");});
        self.rule_editor(ctx);
        self.ids_editor(ctx);
        let mut run_quality = false;
        egui::Window::new("Datakwaliteit").open(&mut self.quality_open).show(ctx,|ui|{
            ui.label("Namen, unieke GUIDs en specifieke IFC-entiteiten worden altijd gecontroleerd.");
            ui.checkbox(&mut self.prefs.quality.material,"Materiaal vereist");
            ui.checkbox(&mut self.prefs.quality.classification,"Classificatie vereist");
            ui.checkbox(&mut self.prefs.quality.nlsfb,"NL-SfB-classificatie vereist (aanwezigheid)");
            ui.checkbox(&mut self.prefs.quality.common_properties,"IsExternal, LoadBearing en FireRating vereist");
            ui.label("Score = geslaagde controles / uitgevoerde controles op objecten met geometrie. Alle controles tellen even zwaar. Geen gecertificeerde ILS- of normtoets.");
            run_quality=ui.add_enabled(!self.models.is_empty(),egui::Button::new("Controle uitvoeren")).clicked();
        });
        if run_quality {
            self.command("quality", ctx);
        }
        if self.clash_rx.is_some() {
            egui::Window::new("Clashcontrole")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.spinner();
                    ui.label("IfcLite vergelijkt de zichtbare meshes…");
                });
        }
        if self.rx.is_some() || self.clash_rx.is_some() || self.cloud_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if cfg!(feature = "screenshot") && std::env::var_os("OPENAEC_SCREENSHOT_TO").is_some() {
            return;
        }
        eframe::set_value(storage, "studio-preferences-v1", &self.prefs);
    }
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.renderer.lock().unwrap().destroy(&self.gl);
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;
    fn view() -> View {
        View {
            name: "Doorsnede".into(),
            camera: Camera::default(),
            hidden: HashSet::from([(0, 42), (1, 42)]),
            clip: true,
            clip_min: Vec3::ZERO,
            clip_max: Vec3::ONE,
            mode: 2,
            edges: true,
        }
    }
    #[test]
    fn project_roundtrip_preserves_model_scoped_visibility() {
        let p = Project {
            version: 1,
            models: vec!["a.ifc".into(), "b.ifc".into()],
            view: view(),
            views: vec![view()],
            rules: vec![crate::clash::Rule::default()],
            topics: vec![],
            findings: vec![],
            clouds: vec![],
        };
        let bytes = serde_json::to_vec(&p).unwrap();
        let restored: Project = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.models, p.models);
        assert_eq!(restored.view.hidden, p.view.hidden);
        assert!(restored.view.clip);
        assert_eq!(restored.view.mode, 2);
        assert!(valid_view(&restored.view));
    }
    #[test]
    fn invalid_camera_and_inverted_clip_are_rejected() {
        let mut v = view();
        v.camera.distance = 0.;
        assert!(!valid_view(&v));
        v.camera.distance = 5.;
        v.clip_min.x = 2.;
        assert!(!valid_view(&v));
    }
}
