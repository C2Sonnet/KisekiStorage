#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(not(target_os = "windows"))]
compile_error!("このプログラムはWindows 10/11専用です。");

mod categories;
mod system;
mod ui;

fn main() -> eframe::Result<()> {
    let title = format!("キセキの倉庫 - Kiseki Storage (v{})", env!("CARGO_PKG_VERSION"));
    // 埋め込み画像が万一破損しても、アイコンなしでUI起動を試みる。
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(title.clone())
        .with_inner_size([1400.0, 900.0])
        .with_min_inner_size([1160.0, 700.0]);
    if let Ok(source) = image::load_from_memory(include_bytes!("../assets/512.png")) {
        let icon = image::imageops::resize(
            &source.to_rgba8(), 256, 256, image::imageops::FilterType::Lanczos3);
        let (width, height) = icon.dimensions();
        viewport = viewport.with_icon(eframe::egui::IconData {
            rgba: icon.into_raw(), width, height,
        });
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        &title,
        options,
        Box::new(|cc| Ok(Box::new(ui::Dashboard::new(cc)))),
    )
}
