mod app;
mod bcf;
mod bcf_topics;
mod camera;
mod clash;
mod cli;
mod data;
mod export;
mod inspection;
mod issues;
mod model;
mod pointcloud;
mod quality;
mod renderer;
mod report;
mod service;
mod theme;
mod validation;
mod view_cube;
mod widgets;

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(result) = cli::run(&args) {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let initial = args.first().map(std::path::PathBuf::from);
    let mut window_size = [1440.0, 900.0];
    if cfg!(feature = "screenshot") && std::env::var_os("OPENAEC_SCREENSHOT_COMPACT").is_some() {
        window_size = [1024.0, 768.0];
    }
    eframe::run_native(
        "OpenAEC · BIM Validator",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!(
                        "../assets/openaec-symbol.png"
                    ))
                    .expect("OpenAEC icon"),
                )
                .with_inner_size(window_size)
                .with_min_inner_size([900.0, 600.0]),
            persist_window: !(cfg!(feature = "screenshot")
                && std::env::var_os("OPENAEC_SCREENSHOT_TO").is_some()),
            persistence_path: if cfg!(feature = "screenshot")
                && std::env::var_os("OPENAEC_SCREENSHOT_TO").is_some()
            {
                Some(
                    std::env::temp_dir()
                        .join(format!("openaec-ui-review-{}.ron", std::process::id())),
                )
            } else {
                None
            },
            depth_buffer: 24,
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(app::BimApp::new(cc, initial)))),
    )
}
