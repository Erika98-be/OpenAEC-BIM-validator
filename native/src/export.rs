//! Standalone exports from the visible native geometry; no web runtime.
use crate::model::Model;
use crate::renderer::Key;
use glam::DVec3;
use serde_json::json;
use std::collections::HashSet;

/// GLB coordinates are Y-up metres. The large world origin is stored in scene extras.
pub fn glb(models: &[Model], hidden: &HashSet<Key>) -> Result<Vec<u8>, String> {
    let meshes: Vec<_> = models
        .iter()
        .enumerate()
        .flat_map(|(mi, m)| {
            m.meshes
                .iter()
                .filter(move |mesh| !hidden.contains(&(mi, mesh.id)))
                .map(move |mesh| (mi, mesh))
        })
        .collect();
    let origin = meshes.first().ok_or("Geen zichtbare geometrie")?.1.origin;
    let mut binary = Vec::new();
    let mut views = vec![];
    let mut accessors = vec![];
    let mut gltf_meshes = vec![];
    let mut materials = vec![];
    let mut nodes = vec![];
    for (mi, mesh) in meshes {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            continue;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let offset = binary.len();
        for v in &mesh.vertices {
            let p = mesh.origin - origin + DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
            let coordinates = [p.x as f32, p.z as f32, -p.y as f32];
            for (axis, value) in coordinates.into_iter().enumerate() {
                if !value.is_finite() {
                    return Err("Niet-eindige geometrie in export".into());
                }
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
                binary.extend(value.to_le_bytes());
            }
        }
        let pv = views.len();
        views.push(
            json!({"buffer":0,"byteOffset":offset,"byteLength":binary.len()-offset,"target":34962}),
        );
        let pa = accessors.len();
        accessors.push(json!({"bufferView":pv,"componentType":5126,"count":mesh.vertices.len(),"type":"VEC3","min":min,"max":max}));
        let offset = binary.len();
        for v in &mesh.vertices {
            for value in [v[3], v[5], -v[4]] {
                binary.extend(value.to_le_bytes());
            }
        }
        let nv = views.len();
        views.push(
            json!({"buffer":0,"byteOffset":offset,"byteLength":binary.len()-offset,"target":34962}),
        );
        let na = accessors.len();
        accessors.push(
            json!({"bufferView":nv,"componentType":5126,"count":mesh.vertices.len(),"type":"VEC3"}),
        );
        let offset = binary.len();
        for index in &mesh.indices {
            if *index as usize >= mesh.vertices.len() {
                return Err("Ongeldige driehoekindex".into());
            }
            binary.extend(index.to_le_bytes());
        }
        let iv = views.len();
        views.push(
            json!({"buffer":0,"byteOffset":offset,"byteLength":binary.len()-offset,"target":34963}),
        );
        let ia = accessors.len();
        accessors.push(json!({"bufferView":iv,"componentType":5125,"count":mesh.indices.len(),"type":"SCALAR"}));
        let material = materials.len();
        materials.push(json!({"pbrMetallicRoughness":{"baseColorFactor":[mesh.color[0],mesh.color[1],mesh.color[2],1.0],"metallicFactor":0.0,"roughnessFactor":0.8},"doubleSided":true}));
        let e = models[mi].elements.get(&mesh.id);
        let name = e.map(|e| e.name.as_str()).unwrap_or("IFC object");
        nodes.push(json!({"mesh":gltf_meshes.len(),"name":name,"extras":{"model":models[mi].name,"expressId":mesh.id,"globalId":e.map(|e|e.guid.as_str())}}));
        gltf_meshes.push(json!({"name":name,"primitives":[{"attributes":{"POSITION":pa,"NORMAL":na},"indices":ia,"material":material}]}));
    }
    if nodes.is_empty() {
        return Err("Geen exporteerbare driehoeken".into());
    }
    let doc = json!({"asset":{"version":"2.0","generator":"OpenAEC BIM Validator"},"scene":0,"scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>(),"extras":{"ifcWorldOriginZUpMetres":origin.to_array()}}],"nodes":nodes,"meshes":gltf_meshes,"materials":materials,"buffers":[{"byteLength":binary.len()}],"bufferViews":views,"accessors":accessors});
    let mut json = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let length =
        u32::try_from(28 + json.len() + binary.len()).map_err(|_| "GLB is groter dan 4 GB")?;
    let mut out = Vec::new();
    for n in [0x46546c67, 2, length, json.len() as u32, 0x4e4f534a] {
        out.extend(u32::to_le_bytes(n));
    }
    out.extend(json);
    out.extend((binary.len() as u32).to_le_bytes());
    out.extend(0x004e4942_u32.to_le_bytes());
    out.extend(binary);
    Ok(out)
}

/// A horizontal triangle-plane intersection, in source XY metres. No fictitious fills/caps.
pub fn section_svg(models: &[Model], hidden: &HashSet<Key>, height: f64) -> Result<String, String> {
    if !height.is_finite() {
        return Err("Ongeldige snedehoogte".into());
    }
    let mut segments = vec![];
    for (mi, m) in models.iter().enumerate() {
        for mesh in &m.meshes {
            if hidden.contains(&(mi, mesh.id)) || height < mesh.min.z || height > mesh.max.z {
                continue;
            }
            for tri in mesh.indices.as_chunks::<3>().0 {
                let points = tri
                    .iter()
                    .map(|&i| {
                        let v = mesh.vertices[i as usize];
                        mesh.origin + DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
                    })
                    .collect::<Vec<_>>();
                let mut hits = vec![];
                for i in 0..3 {
                    let a = points[i];
                    let b = points[(i + 1) % 3];
                    if (a.z <= height && b.z > height) || (b.z <= height && a.z > height) {
                        hits.push(a + (b - a) * ((height - a.z) / (b.z - a.z)));
                    }
                }
                if hits.len() == 2 && hits[0].distance(hits[1]) > 1e-8 {
                    segments.push((hits[0], hits[1]));
                }
            }
        }
    }
    if segments.is_empty() {
        return Err("Geen geometrie op deze snedehoogte".into());
    }
    let min = segments
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .fold(DVec3::splat(f64::INFINITY), DVec3::min);
    let max = segments
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
    let width = (max.x - min.x).max(0.01);
    let depth = (max.y - min.y).max(0.01);
    let margin = width.max(depth) * 0.02;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\"><title>OpenAEC — horizontale snede op {height:.3} m</title><desc>XY-origin {:.6}, {:.6} m. Eén SVG-eenheid = één meter. Z-up IFC coördinaten; Y voor weergave omgekeerd.</desc><g fill=\"none\" stroke=\"#36363e\" stroke-width=\"{}\">",-margin,-margin,width+2.*margin,depth+2.*margin,min.x,max.y,width.max(depth)/1500.);
    for (a, b) in segments {
        svg.push_str(&format!(
            "<path d=\"M {:.6} {:.6} L {:.6} {:.6}\"/>",
            a.x - min.x,
            max.y - a.y,
            b.x - min.x,
            max.y - b.y
        ));
    }
    svg.push_str("</g></svg>");
    Ok(svg)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Model {
        Model::load(
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../test/fixtures/sample.ifc"
            )),
            |_| {},
        )
        .unwrap()
    }
    #[test]
    fn glb_chunk_lengths_indices_and_origin_are_consistent() {
        let m = model();
        let expected = m.meshes.len();
        let bytes = glb(&[m], &HashSet::new()).unwrap();
        let u = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        assert_eq!(u(0), 0x46546c67);
        assert_eq!(u(8), bytes.len());
        let len = u(12);
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        assert_eq!(json["nodes"].as_array().unwrap().len(), expected);
        assert_eq!(
            json["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
            u(20 + len)
        );
        assert_eq!(28 + len + u(20 + len), bytes.len());
    }
    #[test]
    fn slice_is_xml_and_empty_slice_fails() {
        let m = model();
        let z = (m.meshes[0].min.z + m.meshes[0].max.z) * 0.5;
        let models = vec![m];
        let svg = section_svg(&models, &HashSet::new(), z).unwrap();
        assert!(roxmltree::Document::parse(&svg).is_ok());
        assert!(svg.contains("<path"));
        assert!(section_svg(&models, &HashSet::new(), 1e9).is_err());
    }
}
