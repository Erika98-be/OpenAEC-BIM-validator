//! Local BCF 2.1 export, with stable IFC GUID references and a native camera.
use crate::{camera::Camera, model::Model, renderer::Key, validation::Finding};
use glam::Vec3;
use std::{
    collections::HashSet,
    io::{Seek, Write},
};
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn point(name: &str, p: Vec3) -> String {
    format!(
        "<{name}><X>{}</X><Y>{}</Y><Z>{}</Z></{name}>",
        p.x, p.y, p.z
    )
}
fn component(models: &[Model], key: Key) -> Option<String> {
    let e = models.get(key.0)?.elements.get(&key.1)?;
    (!e.guid.is_empty()).then(|| format!("<Component IfcGuid=\"{}\"/>", xml(&e.guid)))
}
pub fn export<W: Write + Seek>(
    writer: W,
    models: &[Model],
    findings: &[Finding],
    camera: &Camera,
    selected: &HashSet<Key>,
    hidden: &HashSet<Key>,
) -> Result<(), String> {
    let mut zip = zip::ZipWriter::new(writer);
    let mut write = |name: &str, body: String| -> Result<(), String> {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };
    write("bcf.version",r#"<?xml version="1.0" encoding="UTF-8"?><Version VersionId="2.1"><DetailedVersion>2.1</DetailedVersion></Version>"#.into())?;
    let issues: Vec<_> = findings
        .iter()
        .filter(|r| matches!(r.severity.as_str(), "Fout" | "Waarschuwing"))
        .collect();
    let topics: Vec<(String, String, HashSet<Key>)> = if issues.is_empty() {
        vec![(
            "Modelinspectie".into(),
            "Standpunt uit OpenAEC BIM Validator".into(),
            selected.clone(),
        )]
    } else {
        issues
            .into_iter()
            .map(|r| {
                (
                    r.rule.clone(),
                    r.message.clone(),
                    r.key().into_iter().chain(r.related).collect(),
                )
            })
            .collect()
    };
    for (title, description, selection) in topics {
        let topic = uuid::Uuid::new_v4();
        let view = uuid::Uuid::new_v4();
        write(
            &format!("{topic}/markup.bcf"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Markup><Topic Guid="{topic}" TopicType="Issue" TopicStatus="Open"><Title>{}</Title><CreationDate>{}</CreationDate><CreationAuthor>OpenAEC BIM Validator</CreationAuthor><Description>{}</Description></Topic><Viewpoints Guid="{view}"><Viewpoint>viewpoint.bcfv</Viewpoint></Viewpoints></Markup>"#,
                xml(&title),
                chrono::Utc::now().to_rfc3339(),
                xml(&description)
            ),
        )?;
        let direction = -camera.direction();
        let right = direction.cross(Vec3::Z).normalize();
        let up = right.cross(direction);
        let frame = format!(
            "{}{}{}",
            point("CameraViewPoint", camera.eye()),
            point("CameraDirection", direction),
            point("CameraUpVector", up)
        );
        let camera_xml = if camera.perspective {
            format!(
                "<PerspectiveCamera>{frame}<FieldOfView>{}</FieldOfView></PerspectiveCamera>",
                camera.fov
            )
        } else {
            format!("<OrthogonalCamera>{frame}<ViewToWorldScale>{}</ViewToWorldScale></OrthogonalCamera>",2.*camera.distance*(camera.fov.to_radians()*0.5).tan())
        };
        let selection_xml = selection
            .iter()
            .filter_map(|k| component(models, *k))
            .collect::<String>();
        let hidden_xml = hidden
            .iter()
            .filter(|k| !selection.contains(k))
            .filter_map(|k| component(models, *k))
            .collect::<String>();
        write(
            &format!("{topic}/viewpoint.bcfv"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><VisualizationInfo Guid="{view}"><Components><Selection>{selection_xml}</Selection><Visibility DefaultVisibility="true"><Exceptions>{hidden_xml}</Exceptions></Visibility></Components>{camera_xml}</VisualizationInfo>"#
            ),
        )?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_contains_valid_xml_and_guid_selection() {
        let m = Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap();
        let mut buffer = std::io::Cursor::new(Vec::new());
        export(
            &mut buffer,
            &[m],
            &[],
            &Camera::default(),
            &HashSet::from([(0, 42)]),
            &HashSet::new(),
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(buffer.into_inner())).unwrap();
        assert_eq!(archive.len(), 3);
        let mut selected = false;
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let mut s = String::new();
            std::io::Read::read_to_string(&mut f, &mut s).unwrap();
            roxmltree::Document::parse(&s).unwrap();
            selected |= s.contains("8XyZ9A0bC12DeFgHiJkLmN");
        }
        assert!(selected);
    }
}
