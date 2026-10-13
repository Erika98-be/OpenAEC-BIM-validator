//! Shared native operations for REST, MCP and the desktop integration panel.
pub mod http;
pub mod mcp;
use crate::{data, model::Model, renderer::Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone)]
pub struct Service {
    pub state: Arc<Mutex<State>>,
    pub workers: Arc<tokio::sync::Semaphore>,
}
pub struct Snapshot {
    pub models: Vec<Model>,
    pub revision: u64,
}
pub struct State {
    pub snapshot: Arc<Snapshot>,
    pub ui: bool,
    pub selection: Vec<Key>,
    pub pending: VecDeque<Selection>,
    pub last_selection: Option<Value>,
    pub audit: VecDeque<Audit>,
    artifacts: HashMap<String, Artifact>,
}
#[derive(Clone)]
pub struct Artifact {
    pub mime: String,
    pub bytes: Arc<Vec<u8>>,
}
pub struct Selection {
    pub id: String,
    pub revision: u64,
    pub keys: Vec<Key>,
    pub focus: bool,
}
#[derive(Clone, Serialize)]
pub struct Audit {
    pub tool: String,
    pub ok: bool,
    pub elapsed_ms: u128,
    pub time: String,
}
#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub message: String,
}
impl Error {
    fn bad(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: message.into(),
        }
    }
    fn missing(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: message.into(),
        }
    }
}
type Result<T> = std::result::Result<T, Error>;
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Query {
    #[serde(default)]
    model: Option<usize>,
    #[serde(default)]
    class: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    property: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    operator: Operator,
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
}
fn page_size() -> usize {
    100
}
#[derive(Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Operator {
    #[default]
    Contains,
    Equals,
    Missing,
    Present,
    Greater,
    Less,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Object {
    model: usize,
    element: u32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Validate {
    #[serde(default)]
    ids_xml: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Clash {
    #[serde(default)]
    model_a: Option<usize>,
    #[serde(default)]
    model_b: Option<usize>,
    #[serde(default)]
    class_a: String,
    #[serde(default)]
    class_b: String,
    #[serde(default)]
    clearance: bool,
    #[serde(default = "tolerance")]
    tolerance: f64,
    #[serde(default = "distance")]
    distance: f64,
}
fn tolerance() -> f64 {
    0.001
}
fn distance() -> f64 {
    0.05
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Diff {
    before: usize,
    after: usize,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    format: Format,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    column: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Format {
    Glb,
    Svg,
    Csv,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Select {
    revision: u64,
    objects: Vec<Object>,
    #[serde(default)]
    focus: bool,
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| Error::bad(e.to_string()))
}
fn definition<T: JsonSchema>(name: &str, description: &str, read_only: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":schemars::generate::SchemaSettings::default().with(|s|s.inline_subschemas=true).into_generator().into_root_schema_for::<T>(),"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}})
}
pub fn tools() -> Vec<Value> {
    vec![
    definition::<Empty>("models_list","List loaded IFC models, counts and current revision. No filesystem access.",true),
    definition::<Query>("objects_query","Filter loaded objects by model, exact IFC type, text and property. Paginated; maximum 1000 rows. Property names use Pset.Name syntax.",true),
    definition::<Object>("object_get","Read attributes, property sets, quantities, materials and classifications of an object.",true),
    definition::<Validate>("validate","Run baseline quality checks or the partial IDS evaluator. Unsupported IDS requirements never pass.",true),
    definition::<Clash>("clash_check","Check loaded geometry for hard clashes or clearance. Optional model/type groups; without group B, compare A with itself. Ignores viewport clipping and hidden objects.",true),
    definition::<Diff>("models_diff","Compare two models by GlobalId, metadata and triangle meshes quantized to a 1 mm grid.",true),
    definition::<Export>("model_export","Export all loaded geometry as GLB, horizontal section as SVG (height required), or objects as CSV. Returns artifact id; never writes filesystem files.",false),
    definition::<Empty>("view_state","Get the desktop selection, queued-request status and model revision. Headless mode reports ui=false.",true),
    definition::<Select>("view_select","Queue object selection and optional camera focus in the connected desktop. Requires current models_list revision. Check view_state for applied status.",false),
    definition::<ArtifactInput>("artifact_read","Read one export artifact as base64 (maximum 4 MiB); larger exports use authenticated REST download.",true),
]
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArtifactInput {
    id: String,
}
impl Service {
    pub fn new(models: Vec<Model>, ui: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                snapshot: Arc::new(Snapshot {
                    models,
                    // A new server instance must reject selections from an older
                    // instance even when it reuses the same HTTP port and token.
                    // Stay in the exactly representable JavaScript integer range.
                    revision: (uuid::Uuid::new_v4().as_u128() as u64) & ((1_u64 << 52) - 1),
                }),
                ui,
                selection: vec![],
                pending: VecDeque::new(),
                last_selection: None,
                audit: VecDeque::new(),
                artifacts: HashMap::new(),
            })),
            workers: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
    pub fn sync(&self, models: &[Model]) {
        let mut state = self.state.lock().unwrap();
        let revision = state.snapshot.revision + 1;
        state.snapshot = Arc::new(Snapshot {
            models: models.to_vec(),
            revision,
        });
        state.pending.clear();
        state.selection.clear();
        state.last_selection = None;
        state.artifacts.clear();
    }
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.state.lock().unwrap().snapshot.clone()
    }
    pub fn artifact(&self, id: &str) -> Result<Artifact> {
        self.state
            .lock()
            .unwrap()
            .artifacts
            .get(id)
            .cloned()
            .ok_or_else(|| Error::missing("Artifact not found or expired"))
    }
    pub async fn call(&self, name: String, args: Value) -> Result<Value> {
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error {
                status: 429,
                message: "Two operations are already running; retry later".into(),
            })?;
        let service = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let start = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                service.execute(&name, args)
            }))
            .unwrap_or_else(|_| {
                Err(Error {
                    status: 500,
                    message: "Native operation failed".into(),
                })
            });
            let mut state = service.state.lock().unwrap();
            if state.audit.len() >= 100 {
                state.audit.pop_front();
            }
            state.audit.push_back(Audit {
                tool: name,
                ok: result.is_ok(),
                elapsed_ms: start.elapsed().as_millis(),
                time: chrono::Utc::now().to_rfc3339(),
            });
            result
        })
        .await
        .map_err(|_| Error {
            status: 500,
            message: "Worker stopped".into(),
        })?
    }
    fn execute(&self, name: &str, args: Value) -> Result<Value> {
        let snapshot = self.snapshot();
        let models = &snapshot.models;
        let get = |index: usize| {
            models
                .get(index)
                .ok_or_else(|| Error::missing("Model not found"))
        };
        match name {
            "models_list" => {
                decode::<Empty>(args)?;
                Ok(
                    json!({"revision":snapshot.revision,"models":models.iter().enumerate().map(|(i,m)|json!({"id":i,"name":m.name,"schema":m.schema,"objects":m.elements.len(),"meshes":m.meshes.len(),"triangles":m.triangle_count(),"warnings":m.warnings})).collect::<Vec<_>>()}),
                )
            }
            "objects_query" => {
                let q: Query = decode(args)?;
                if q.limit == 0 || q.limit > 1000 {
                    return Err(Error::bad("limit must be 1..1000"));
                }
                if let Some(i) = q.model {
                    get(i)?;
                }
                if matches!(q.operator, Operator::Greater | Operator::Less)
                    && q.value.parse::<f64>().ok().is_none_or(|v| !v.is_finite())
                {
                    return Err(Error::bad("Numeric filter requires a finite numeric value"));
                }
                let filter = data::Query {
                    text: q.text,
                    class: q.class,
                    property: q.property,
                    value: q.value,
                    operator: match q.operator {
                        Operator::Contains => 0,
                        Operator::Equals => 1,
                        Operator::Missing => 2,
                        Operator::Present => 3,
                        Operator::Greater => 4,
                        Operator::Less => 5,
                    },
                };
                let rows: Vec<_> = filter
                    .rows(models)
                    .into_iter()
                    .filter(|(mi, _)| q.model.is_none_or(|i| i == *mi))
                    .collect();
                let total = rows.len();
                let objects = rows
                    .into_iter()
                    .skip(q.offset)
                    .take(q.limit)
                    .map(|(mi, id)| object_json(mi, &models[mi].elements[&id]))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"revision":snapshot.revision,"total":total,"offset":q.offset,"objects":objects}),
                )
            }
            "object_get" => {
                let p: Object = decode(args)?;
                let e = get(p.model)?
                    .elements
                    .get(&p.element)
                    .ok_or_else(|| Error::missing("Object not found"))?;
                Ok(json!({"revision":snapshot.revision,"object":object_json(p.model,e)}))
            }
            "validate" => {
                let p: Validate = decode(args)?;
                let findings = if let Some(xml) = p.ids_xml {
                    if xml.len() > 1_000_000 {
                        return Err(Error::bad("IDS exceeds 1 MB"));
                    }
                    crate::validation::check_ids(models, &xml).map_err(Error::bad)?
                } else {
                    crate::validation::check_models(models)
                };
                Ok(
                    json!({"revision":snapshot.revision,"findings":findings,"idsCoverage":"partial"}),
                )
            }
            "clash_check" => {
                let p: Clash = decode(args)?;
                for i in [p.model_a, p.model_b].into_iter().flatten() {
                    get(i)?;
                }
                let rule = crate::clash::Rule {
                    name: "API clash check".into(),
                    model_a: p.model_a,
                    model_b: p.model_b,
                    class_a: p.class_a,
                    class_b: p.class_b,
                    clearance: p.clearance,
                    tolerance: p.tolerance,
                    distance: p.distance,
                    ..Default::default()
                };
                if !rule.valid() {
                    return Err(Error::bad("Distances must be finite and nonnegative"));
                }
                let mut meshes = vec![];
                let mut a = vec![];
                let mut b = vec![];
                let two = rule.model_b.is_some() || !rule.class_b.is_empty();
                for (mi, m) in models.iter().enumerate() {
                    for mesh in &m.meshes {
                        let class = m
                            .elements
                            .get(&mesh.id)
                            .map(|e| e.class.as_str())
                            .unwrap_or("");
                        let in_a = rule.model_a.is_none_or(|i| i == mi)
                            && (rule.class_a.is_empty() || class == rule.class_a);
                        let in_b = two
                            && rule.model_b.is_none_or(|i| i == mi)
                            && (rule.class_b.is_empty() || class == rule.class_b);
                        if in_a || in_b {
                            let index = meshes.len() as u32;
                            if in_a {
                                a.push(index);
                            }
                            if in_b {
                                b.push(index);
                            }
                            meshes.push(((mi, mesh.id), mesh.clone()));
                        }
                    }
                }
                let findings =
                    crate::clash::check_rule(meshes, &a, two.then_some(b.as_slice()), &rule);
                Ok(json!({"revision":snapshot.revision,"findings":findings}))
            }
            "models_diff" => {
                let p: Diff = decode(args)?;
                get(p.before)?;
                get(p.after)?;
                Ok(
                    json!({"revision":snapshot.revision,"differences":data::compare(models,p.before,p.after).map_err(Error::bad)?}),
                )
            }
            "model_export" => {
                let p: Export = decode(args)?;
                let (mime, bytes) = match p.format {
                    Format::Glb => (
                        "model/gltf-binary",
                        crate::export::glb(models, &HashSet::new()).map_err(Error::bad)?,
                    ),
                    Format::Svg => (
                        "image/svg+xml",
                        crate::export::section_svg(
                            models,
                            &HashSet::new(),
                            p.height.ok_or_else(|| {
                                Error::bad("SVG export requires height in metres")
                            })?,
                        )
                        .map_err(Error::bad)?
                        .into_bytes(),
                    ),
                    Format::Csv => {
                        let rows = data::Query::default().rows(models);
                        (
                            "text/csv; charset=utf-8",
                            data::csv(models, &rows, p.column.as_deref().unwrap_or("Material"))
                                .into_bytes(),
                        )
                    }
                };
                if bytes.len() > 64 * 1024 * 1024 {
                    return Err(Error::bad("Export exceeds 64 MiB artifact limit"));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let size = bytes.len();
                let mut state = self.state.lock().unwrap();
                if state.snapshot.revision != snapshot.revision {
                    return Err(Error {
                        status: 409,
                        message: "Models changed during export; retry".into(),
                    });
                }
                if state.artifacts.len() >= 8
                    || state
                        .artifacts
                        .values()
                        .map(|a| a.bytes.len())
                        .sum::<usize>()
                        + size
                        > 64 * 1024 * 1024
                {
                    state.artifacts.clear();
                }
                state.artifacts.insert(
                    id.clone(),
                    Artifact {
                        mime: mime.into(),
                        bytes: Arc::new(bytes),
                    },
                );
                Ok(
                    json!({"revision":snapshot.revision,"id":id,"mimeType":mime,"bytes":size,"download":format!("/api/v1/artifacts/{id}"),"readTool":"artifact_read"}),
                )
            }
            "artifact_read" => {
                use base64::Engine;
                let p: ArtifactInput = decode(args)?;
                let a = self.artifact(&p.id)?;
                if a.bytes.len() > 4 * 1024 * 1024 {
                    return Err(Error::bad(
                        "Use the authenticated HTTP download for artifacts larger than 4 MiB",
                    ));
                }
                Ok(
                    json!({"id":p.id,"mimeType":a.mime,"base64":base64::engine::general_purpose::STANDARD.encode(a.bytes.as_slice())}),
                )
            }
            "view_state" => {
                decode::<Empty>(args)?;
                let s = self.state.lock().unwrap();
                Ok(
                    json!({"ui":s.ui,"revision":s.snapshot.revision,"selection":s.selection.iter().map(|(m,e)|json!({"model":m,"element":e})).collect::<Vec<_>>(),"pending":s.pending.len(),"lastRequest":s.last_selection}),
                )
            }
            "view_select" => {
                let p: Select = decode(args)?;
                if p.objects.len() > 1000 {
                    return Err(Error::bad("Select at most 1000 objects"));
                }
                for o in &p.objects {
                    if !get(o.model)?.elements.contains_key(&o.element) {
                        return Err(Error::missing("Selection object not found"));
                    }
                }
                let mut s = self.state.lock().unwrap();
                if !s.ui {
                    return Err(Error {
                        status: 409,
                        message: "No desktop UI attached".into(),
                    });
                }
                if p.revision != snapshot.revision || p.revision != s.snapshot.revision {
                    return Err(Error {
                        status: 409,
                        message: "Model revision changed; refresh models_list".into(),
                    });
                }
                if s.pending.len() >= 32 {
                    return Err(Error {
                        status: 429,
                        message: "Desktop command queue is full".into(),
                    });
                }
                let id = uuid::Uuid::new_v4().to_string();
                s.pending.push_back(Selection {
                    id: id.clone(),
                    revision: p.revision,
                    keys: p
                        .objects
                        .into_iter()
                        .map(|o| (o.model, o.element))
                        .collect(),
                    focus: p.focus,
                });
                Ok(json!({"requestId":id,"status":"queued"}))
            }
            _ => Err(Error::missing("Unknown tool")),
        }
    }
}
fn object_json(model: usize, e: &crate::model::Element) -> Value {
    json!({"model":model,"element":e.id,"globalId":e.guid,"type":e.class,"name":e.name,"parent":e.parent,"attributes":e.attributes,"properties":e.properties,"propertyTypes":e.property_types,"materials":e.materials,"classifications":e.classifications})
}

#[cfg(test)]
mod tests;
