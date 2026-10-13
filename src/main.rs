#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(not(target_os = "windows"))]
compile_error!("このプログラムはWindows 10/11専用です。");

mod categories;
mod system;
mod ui;

fn main() -> eframe::Result<()> {
    let title = format!("キセキの倉庫 - Kiseki Storage (v{})", env!("CARGO_PKG_VERSION"));
    // アップロードされた512.pngのアルファを保持し、ウィンドウアイコンに使用。
    let source_icon = image::load_from_memory(include_bytes!("../assets/512.png"))
        .expect("512.png の読み込みに失敗しました")
        .to_rgba8();
    let icon = image::imageops::resize(
        &source_icon, 256, 256, image::imageops::FilterType::Lanczos3);
    let (width, height) = icon.dimensions();
    let icon_data = eframe::egui::IconData { rgba: icon.into_raw(), width, height };
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title(title.clone())
        .with_icon(icon_data)
        .with_inner_size([1400.0, 900.0])
        .with_min_inner_size([900.0, 650.0]);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        &title,
        options,
        Box::new(|cc| Ok(Box::new(ui::Dashboard::new(cc)))),
    )
}
