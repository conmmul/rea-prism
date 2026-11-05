//! A desktop reader, editor and graph renderer for GraphPad Prism documents.

mod app;
mod dialogs;
mod grid;
mod inspector;
mod paint;
mod widgets;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Prism"),
        ..Default::default()
    };

    eframe::run_native(
        "rea-prism",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, open)))),
    )
}
