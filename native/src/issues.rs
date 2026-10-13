//! Durable coordination topics. IFC GUIDs survive STEP-id changes on reload.
use crate::{camera::Camera, model::Model, renderer::Key, validation::Finding};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub date: String,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Topic {
    #[serde(default)]
    pub snapshot: Vec<u8>,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default = "visible")]
    pub default_visibility: bool,
    #[serde(default)]
    pub clipping: Vec<ClipPlane>,
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub category: String,
    pub assignee: String,
    pub created: String,
    pub modified: String,
    pub author: String,
    pub components: Vec<String>,
    pub comments: Vec<Comment>,
    pub camera: Option<Camera>,
    #[serde(default)]
    pub source: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ClipPlane {
    pub location: glam::Vec3,
    pub direction: glam::Vec3,
}
fn visible() -> bool {
    true
}
impl Topic {
    pub fn new(title: String, components: Vec<String>, camera: Option<Camera>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            snapshot: vec![],
            hidden: vec![],
            default_visibility: true,
            clipping: vec![],
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description: String::new(),
            status: "Open".into(),
            priority: "Normal".into(),
            category: "Issue".into(),
            assignee: String::new(),
            created: now.clone(),
            modified: now,
            author: "OpenAEC BIM Validator".into(),
            components,
            comments: vec![],
            camera,
            source: String::new(),
        }
    }
    pub fn touch(&mut self) {
        self.modified = chrono::Utc::now().to_rfc3339();
    }
    pub fn comment(&mut self, author: String, text: String) {
        if text.trim().is_empty() {
            return;
        }
        self.comments.push(Comment {
            id: uuid::Uuid::new_v4().to_string(),
            author,
            date: chrono::Utc::now().to_rfc3339(),
            text,
        });
        self.touch();
    }
    pub fn keys(&self, models: &[Model]) -> Vec<Key> {
        models
            .iter()
            .enumerate()
            .flat_map(|(mi, m)| {
                m.elements
                    .values()
                    .filter(|e| self.components.contains(&e.guid))
                    .map(move |e| (mi, e.id))
            })
            .collect()
    }
}
pub fn guids(models: &[Model], keys: impl Iterator<Item = Key>) -> Vec<String> {
    let mut result: Vec<_> = keys
        .filter_map(|(mi, id)| models.get(mi)?.elements.get(&id))
        .filter(|e| !e.guid.is_empty())
        .map(|e| e.guid.clone())
        .collect();
    result.sort();
    result.dedup();
    result
}
/// Repeat conversion updates the original topic without resetting user triage.
pub fn reconcile(
    topics: &mut Vec<Topic>,
    findings: &[Finding],
    models: &[Model],
    camera: &Camera,
) -> usize {
    let mut added = 0;
    for f in findings
        .iter()
        .filter(|f| matches!(f.severity.as_str(), "Fout" | "Waarschuwing"))
    {
        let components = guids(models, f.key().into_iter().chain(f.related));
        // Findings without a GUID use their message to avoid merging unrelated imports.
        let source = serde_json::to_string(&(
            f.rule.as_str(),
            &components,
            if components.is_empty() {
                f.message.as_str()
            } else {
                ""
            },
        ))
        .unwrap();
        if let Some(topic) = topics.iter_mut().find(|t| t.source == source) {
            topic.description = f.message.clone();
            topic.touch();
        } else {
            let mut topic = Topic::new(f.rule.clone(), components, Some(camera.clone()));
            topic.source = source;
            topic.description = f.message.clone();
            topics.push(topic);
            added += 1;
        }
    }
    added
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_preserves_status_owner_comments_and_id() {
        let f = Finding {
            model: 0,
            element: None,
            related: None,
            rule: "Import".into(),
            message: "Ontbrekende geometrie".into(),
            severity: "Fout".into(),
        };
        let mut topics = vec![];
        assert_eq!(
            reconcile(
                &mut topics,
                std::slice::from_ref(&f),
                &[],
                &Camera::default()
            ),
            1
        );
        let id = topics[0].id.clone();
        topics[0].status = "Resolved".into();
        topics[0].assignee = "BIM-team".into();
        topics[0].comment("Reviewer".into(), "Gecontroleerd".into());
        assert_eq!(reconcile(&mut topics, &[f], &[], &Camera::default()), 0);
        assert_eq!(topics[0].id, id);
        assert_eq!(topics[0].status, "Resolved");
        assert_eq!(topics[0].comments.len(), 1);
        assert_eq!(topics[0].assignee, "BIM-team");
    }
}
