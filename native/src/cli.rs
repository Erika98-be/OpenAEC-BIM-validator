//! Headless access to the same native services used by the desktop UI.
use crate::{data, export, model::Model, validation};
use serde_json::json;
use std::{collections::HashSet, path::Path};

pub fn run(args: &[String]) -> Option<Result<(), String>> {
    let command = args.first()?.as_str();
    if !command.starts_with("--") {
        return None;
    }
    Some(execute(command, &args[1..]))
}
fn execute(command: &str, args: &[String]) -> Result<(), String> {
    const HELP:&str="OpenAEC BIM Validator\n  <model.ifc>                         Desktop openen\n  --inspect <model.ifc>                JSON modelinformatie\n  --query <model.ifc> [IfcType]         JSON objecten en properties\n  --validate <model.ifc>               JSON basiskwaliteitsbevindingen\n  --diff <oud.ifc> <nieuw.ifc>          JSON revisieverschillen\n  --glb <model.ifc> <uitvoer.glb>       3D-export\n  --section <model.ifc> <z-m> <uitvoer.svg>  Horizontale snede\nBestaande uitvoerbestanden worden niet overschreven.";
    if command == "--help" {
        println!("{HELP}");
        return Ok(());
    }
    if ![
        "--inspect",
        "--query",
        "--validate",
        "--diff",
        "--glb",
        "--section",
    ]
    .contains(&command)
    {
        return Err(format!("Onbekend commando: {command}\n{HELP}"));
    }
    let expected = match command {
        "--glb" | "--diff" => 2,
        "--section" => 3,
        _ => 1,
    };
    if args.len() != expected && !(command == "--query" && args.len() == 2) {
        return Err(HELP.into());
    }
    let m = Model::load(Path::new(&args[0]), |_| {})?;
    match command {
        "--inspect" => println!(
            "{}",
            json!({"file":m.name,"schema":m.schema,"objects":m.elements.len(),"meshes":m.meshes.len(),"triangles":m.triangle_count(),"warnings":m.warnings})
        ),
        "--query" => {
            let rows=m.elements.values().filter(|e|args.get(1).is_none_or(|class|class==&e.class)).map(|e|json!({"expressId":e.id,"globalId":e.guid,"type":e.class,"name":e.name,"attributes":e.attributes,"properties":e.properties,"materials":e.materials,"classifications":e.classifications})).collect::<Vec<_>>();
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
            );
        }
        "--validate" => println!(
            "{}",
            serde_json::to_string_pretty(&validation::check_models(&[m]))
                .map_err(|e| e.to_string())?
        ),
        "--diff" => {
            let b = Model::load(Path::new(&args[1]), |_| {})?;
            println!(
                "{}",
                serde_json::to_string_pretty(&data::compare(&[m, b], 0, 1)?)
                    .map_err(|e| e.to_string())?
            );
        }
        "--glb" => write_new(&args[1], &export::glb(&[m], &HashSet::new())?)?,
        "--section" => {
            let height = args[1]
                .parse()
                .map_err(|_| "Z moet een getal in meters zijn")?;
            write_new(
                &args[2],
                export::section_svg(&[m], &HashSet::new(), height)?.as_bytes(),
            )?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
fn write_new(path: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{path}: {e}"))?;
    file.write_all(bytes).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_or_incomplete_command_fails_before_import() {
        assert!(execute("--glb", &[]).is_err());
        assert!(execute("--unknown", &[]).is_err());
        assert!(run(&["model.ifc".into()]).is_none());
    }
}
