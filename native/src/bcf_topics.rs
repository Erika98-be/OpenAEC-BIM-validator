//! BCF 2.1/3.0 topic interchange. ZIP contents are read in memory, never extracted.
use crate::{
    camera::Camera,
    issues::{Comment, Topic},
};
use glam::Vec3;
use std::io::{Read, Seek, Write};
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn text(n: roxmltree::Node<'_, '_>, name: &str) -> String {
    n.children()
        .find(|c| c.has_tag_name(name))
        .and_then(|c| c.text())
        .unwrap_or("")
        .into()
}
fn vector(n: roxmltree::Node<'_, '_>, name: &str) -> Option<Vec3> {
    let p = n.children().find(|c| c.has_tag_name(name))?;
    let v = Vec3::new(
        text(p, "X").parse().ok()?,
        text(p, "Y").parse().ok()?,
        text(p, "Z").parse().ok()?,
    );
    v.is_finite().then_some(v)
}
fn point(name: &str, p: Vec3) -> String {
    format!(
        "<{name}><X>{}</X><Y>{}</Y><Z>{}</Z></{name}>",
        p.x, p.y, p.z
    )
}
fn read<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<String, String> {
    let file = zip.by_name(name).map_err(|e| e.to_string())?;
    if file.size() > 16 * 1024 * 1024 {
        return Err(format!("BCF XML-bestand te groot: {name}"));
    }
    let mut s = String::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    Ok(s)
}
pub fn import<R: Read + Seek>(reader: R) -> Result<Vec<Topic>, String> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    if zip.len() > 100_000 {
        return Err("Te veel bestanden in BCF".into());
    }
    let version = read(&mut zip, "bcf.version")?;
    let doc = roxmltree::Document::parse(&version).map_err(|e| e.to_string())?;
    if !matches!(
        doc.root_element().attribute("VersionId"),
        Some("2.1" | "3.0")
    ) {
        return Err("Ondersteund: BCF 2.1 en 3.0".into());
    }
    let names: Vec<_> = zip
        .file_names()
        .filter(|n| n.ends_with("/markup.bcf"))
        .map(str::to_owned)
        .collect();
    let mut topics = vec![];
    for name in names {
        let source = read(&mut zip, &name)?;
        let doc = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
        let node = doc
            .descendants()
            .find(|n| n.has_tag_name("Topic"))
            .ok_or("BCF Topic ontbreekt")?;
        let id = node.attribute("Guid").ok_or("BCF topic GUID ontbreekt")?;
        uuid::Uuid::parse_str(id).map_err(|_| "Ongeldige BCF topic GUID")?;
        let mut topic = Topic::new(text(node, "Title"), vec![], None);
        topic.id = id.into();
        topic.description = text(node, "Description");
        topic.status = node.attribute("TopicStatus").unwrap_or("Open").into();
        topic.category = node.attribute("TopicType").unwrap_or("Issue").into();
        topic.priority = text(node, "Priority");
        topic.assignee = text(node, "AssignedTo");
        topic.author = text(node, "CreationAuthor");
        topic.created = text(node, "CreationDate");
        topic.modified = text(node, "ModifiedDate");
        if topic.modified.is_empty() {
            topic.modified = topic.created.clone();
        }
        for c in doc
            .descendants()
            .filter(|n| n.has_tag_name("Comment") && n.attribute("Guid").is_some())
        {
            topic.comments.push(Comment {
                id: c.attribute("Guid").unwrap().into(),
                author: text(c, "Author"),
                date: text(c, "Date"),
                text: text(c, "Comment"),
            });
        }
        if let Some(snapshot) = doc
            .descendants()
            .find(|n| n.has_tag_name("Snapshot"))
            .and_then(|n| n.text())
        {
            if snapshot.contains(['/', '\\']) || snapshot == ".." {
                return Err("Ongeldige BCF snapshot-verwijzing".into());
            }
            let folder = name.rsplit_once('/').unwrap().0;
            let file = zip
                .by_name(&format!("{folder}/{snapshot}"))
                .map_err(|e| e.to_string())?;
            if file.size() > 32 * 1024 * 1024 {
                return Err("BCF snapshot te groot".into());
            }
            file.take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut topic.snapshot)
                .map_err(|e| e.to_string())?;
        }
        if let Some(file) = doc
            .descendants()
            .find(|n| n.has_tag_name("Viewpoint") && n.text().is_some())
            .and_then(|n| n.text())
        {
            if file.contains(['/', '\\']) || file == ".." {
                return Err("Ongeldige BCF viewpoint-verwijzing".into());
            }
            let folder = name.rsplit_once('/').unwrap().0;
            let view = read(&mut zip, &format!("{folder}/{file}"))?;
            let doc = roxmltree::Document::parse(&view).map_err(|e| e.to_string())?;
            if let Some(selection) = doc.descendants().find(|n| n.has_tag_name("Selection")) {
                topic.components = selection
                    .children()
                    .filter_map(|n| n.attribute("IfcGuid").map(str::to_owned))
                    .collect();
            }
            if let Some(visibility) = doc.descendants().find(|n| n.has_tag_name("Visibility")) {
                topic.default_visibility = matches!(
                    visibility.attribute("DefaultVisibility"),
                    Some("true" | "1")
                );
                topic.hidden = visibility
                    .descendants()
                    .filter_map(|n| n.attribute("IfcGuid").map(str::to_owned))
                    .collect();
            }
            for plane in doc
                .descendants()
                .filter(|n| n.has_tag_name("ClippingPlane"))
            {
                topic.clipping.push(crate::issues::ClipPlane {
                    location: vector(plane, "Location").ok_or("Ongeldig snijvlak")?,
                    direction: vector(plane, "Direction").ok_or("Ongeldig snijvlak")?,
                });
            }
            if let Some(cam) = doc
                .descendants()
                .find(|n| n.has_tag_name("PerspectiveCamera") || n.has_tag_name("OrthogonalCamera"))
            {
                let eye = vector(cam, "CameraViewPoint").ok_or("Ongeldige BCF camerapositie")?;
                let dir = vector(cam, "CameraDirection")
                    .filter(|v| v.length_squared() > 1e-10)
                    .ok_or("Ongeldige BCF camerarichting")?
                    .normalize();
                let perspective = cam.has_tag_name("PerspectiveCamera");
                let fov = if perspective {
                    text(cam, "FieldOfView")
                        .parse::<f32>()
                        .map_err(|_| "Ongeldig BCF gezichtsveld")?
                } else {
                    45.
                };
                if !fov.is_finite() || !(1.0..179.).contains(&fov) {
                    return Err("Ongeldig BCF gezichtsveld".into());
                }
                let distance = if perspective {
                    20.
                } else {
                    text(cam, "ViewToWorldScale")
                        .parse::<f32>()
                        .map_err(|_| "Ongeldige BCF schaal")?
                        / (2. * (fov.to_radians() * 0.5).tan())
                };
                if !distance.is_finite() || distance <= 0. {
                    return Err("Ongeldige BCF schaal".into());
                }
                let back = -dir;
                topic.camera = Some(Camera {
                    target: eye + dir * distance,
                    distance,
                    yaw: back.y.atan2(back.x),
                    pitch: back.z.atan2(back.x.hypot(back.y)).clamp(-1.56, 1.56),
                    perspective,
                    fov,
                });
            }
        }
        topics.push(topic);
    }
    if topics.is_empty() {
        return Err("Geen topics in BCF-bestand".into());
    }
    Ok(topics)
}
pub fn export<W: Write + Seek>(
    writer: W,
    topics: &[Topic],
    version3: bool,
    aspect: f32,
) -> Result<(), String> {
    let mut zip = zip::ZipWriter::new(writer);
    let mut snapshots = vec![];
    let mut write = |name: &str, body: String| -> Result<(), String> {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };
    write(
        "bcf.version",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Version VersionId=\"{}\"/>",
            if version3 { "3.0" } else { "2.1" }
        ),
    )?;
    if version3 {
        let mut body = "<Extensions>".to_string();
        for (parent, child, values) in [
            (
                "TopicTypes",
                "TopicType",
                topics
                    .iter()
                    .map(|t| t.category.as_str())
                    .collect::<Vec<_>>(),
            ),
            (
                "TopicStatuses",
                "TopicStatus",
                topics.iter().map(|t| t.status.as_str()).collect(),
            ),
            (
                "Priorities",
                "Priority",
                topics.iter().map(|t| t.priority.as_str()).collect(),
            ),
        ] {
            body += &format!("<{parent}>");
            let values: std::collections::BTreeSet<_> =
                values.into_iter().filter(|s| !s.is_empty()).collect();
            for value in values {
                body += &format!("<{child}>{}</{child}>", xml(value));
            }
            body += &format!("</{parent}>");
        }
        body += "</Extensions>";
        write("extensions.xml", body)?;
    }
    for t in topics {
        uuid::Uuid::parse_str(&t.id).map_err(|_| "Ongeldige topic GUID")?;
        if t.title.trim().is_empty() {
            return Err("Een issue heeft geen titel".into());
        }
        let view_id = uuid::Uuid::new_v4().to_string();
        let comments=t.comments.iter().map(|c|format!("<Comment Guid=\"{}\"><Date>{}</Date><Author>{}</Author><Comment>{}</Comment></Comment>",xml(&c.id),xml(&c.date),xml(&c.author),xml(&c.text))).collect::<String>();
        let snapshot = if t.snapshot.is_empty() {
            String::new()
        } else {
            snapshots.push((format!("{}/snapshot.png", t.id), t.snapshot.clone()));
            "<Snapshot>snapshot.png</Snapshot>".into()
        };
        let viewpoint = if t.camera.is_some() {
            format!(
                "<{} Guid=\"{view_id}\"><Viewpoint>viewpoint.bcfv</Viewpoint>{snapshot}</{}>",
                if version3 { "ViewPoint" } else { "Viewpoints" },
                if version3 { "ViewPoint" } else { "Viewpoints" }
            )
        } else {
            String::new()
        };
        let mut topic = format!(
            "<Topic Guid=\"{}\" TopicType=\"{}\" TopicStatus=\"{}\"><Title>{}</Title>",
            xml(&t.id),
            xml(&t.category),
            xml(&t.status),
            xml(&t.title)
        );
        if !t.priority.is_empty() {
            topic += &format!("<Priority>{}</Priority>", xml(&t.priority));
        }
        topic+=&format!("<CreationDate>{}</CreationDate><CreationAuthor>{}</CreationAuthor><ModifiedDate>{}</ModifiedDate>",xml(&t.created),xml(&t.author),xml(&t.modified));
        if !t.assignee.is_empty() {
            topic += &format!("<AssignedTo>{}</AssignedTo>", xml(&t.assignee));
        }
        if !t.description.is_empty() {
            topic += &format!("<Description>{}</Description>", xml(&t.description));
        }
        if version3 {
            topic += &format!(
                "<Comments>{comments}</Comments><Viewpoints>{viewpoint}</Viewpoints></Topic>"
            );
        } else {
            topic += &format!("</Topic>{comments}{viewpoint}");
        }
        write(
            &format!("{}/markup.bcf", t.id),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Markup>{topic}</Markup>"),
        )?;
        if let Some(c) = &t.camera {
            let dir = -c.direction();
            let up = dir.cross(Vec3::Z).normalize().cross(dir);
            let frame = format!(
                "{}{}{}",
                point("CameraViewPoint", c.eye()),
                point("CameraDirection", dir),
                point("CameraUpVector", up)
            );
            let aspect = if version3 {
                format!("<AspectRatio>{}</AspectRatio>", aspect.max(0.1))
            } else {
                String::new()
            };
            let camera = if c.perspective {
                format!("<PerspectiveCamera>{frame}<FieldOfView>{}</FieldOfView>{aspect}</PerspectiveCamera>",c.fov)
            } else {
                format!("<OrthogonalCamera>{frame}<ViewToWorldScale>{}</ViewToWorldScale>{aspect}</OrthogonalCamera>",2.*c.distance*(c.fov.to_radians()*0.5).tan())
            };
            let selection = t
                .components
                .iter()
                .map(|g| format!("<Component IfcGuid=\"{}\"/>", xml(g)))
                .collect::<String>();
            let hidden = t
                .hidden
                .iter()
                .map(|g| format!("<Component IfcGuid=\"{}\"/>", xml(g)))
                .collect::<String>();
            let clipping = t
                .clipping
                .iter()
                .map(|p| {
                    format!(
                        "<ClippingPlane>{}{}</ClippingPlane>",
                        point("Location", p.location),
                        point("Direction", p.direction)
                    )
                })
                .collect::<String>();
            write(&format!("{}/viewpoint.bcfv",t.id),format!("<VisualizationInfo Guid=\"{view_id}\"><Components><Selection>{selection}</Selection><Visibility DefaultVisibility=\"{}\"><Exceptions>{hidden}</Exceptions></Visibility></Components>{camera}<ClippingPlanes>{clipping}</ClippingPlanes></VisualizationInfo>",t.default_visibility))?;
        }
    }
    for (name, bytes) in snapshots {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_versions_roundtrip_topics_comments_and_camera() {
        for version3 in [false, true] {
            let mut topic = Topic::new(
                "Clash <A&B>".into(),
                vec!["0AAAAAAAAAAAAAAAAAAAAA".into()],
                Some(Camera::default()),
            );
            topic.snapshot = include_bytes!("../assets/openaec-symbol.png").to_vec();
            topic.hidden = vec!["1BBBBBBBBBBBBBBBBBBBBB".into()];
            topic.clipping.push(crate::issues::ClipPlane {
                location: Vec3::new(0., 0., 2.),
                direction: Vec3::NEG_Z,
            });
            topic.status = "Resolved".into();
            topic.assignee = "Reviewer".into();
            topic.comment("Auteur".into(), "Maat & controle".into());
            let mut data = std::io::Cursor::new(vec![]);
            export(&mut data, std::slice::from_ref(&topic), version3, 1.8).unwrap();
            if let Ok(dir) = std::env::var("OPENAEC_BCF_TEST_OUTPUT") {
                std::fs::write(
                    std::path::Path::new(&dir).join(if version3 {
                        "3.0.bcfzip"
                    } else {
                        "2.1.bcfzip"
                    }),
                    data.get_ref(),
                )
                .unwrap();
            }
            data.set_position(0);
            let loaded = import(data).unwrap();
            assert_eq!(loaded[0].snapshot, topic.snapshot);
            assert_eq!(loaded[0].hidden, topic.hidden);
            assert_eq!(loaded[0].clipping[0].direction, Vec3::NEG_Z);
            assert_eq!(loaded[0].title, topic.title);
            assert_eq!(loaded[0].id, topic.id);
            assert_eq!(loaded[0].status, "Resolved");
            assert_eq!(loaded[0].components, topic.components);
            assert_eq!(loaded[0].comments[0].text, "Maat & controle");
            assert!(
                (loaded[0].camera.as_ref().unwrap().eye() - topic.camera.unwrap().eye()).length()
                    < 0.001
            );
        }
    }
}
