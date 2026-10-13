//! eframe / egui によるWindows GUI。
use std::{collections::HashMap, path::PathBuf, sync::mpsc::Receiver, time::{Duration, Instant}};
use eframe::egui::{self, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui};
use crate::{categories::{self, Category, CATEGORIES}, system::{self, DeviceInfo, Partition, Update}};

const INK: Color32 = Color32::from_rgb(77, 65, 62);
const MUTED: Color32 = Color32::from_rgb(139, 119, 113);
const BLUE: Color32 = Color32::from_rgb(184, 104, 82);
const MINT: Color32 = Color32::from_rgb(59, 145, 119);
const LILAC: Color32 = Color32::from_rgb(148, 113, 145);
const W: Color32 = Color32::WHITE;

struct ImageSet {
    sun: TextureHandle,
    moon: TextureHandle,
    one: TextureHandle,
    google: TextureHandle,
    c2s: TextureHandle,
    partitions: HashMap<&'static str, TextureHandle>,
}

impl ImageSet {
    fn new(ctx: &egui::Context) -> Self {
        let mut partitions = HashMap::new();
        for cat in CATEGORIES {
            partitions.insert(cat.name, load_texture(ctx, cat.name, cat.icon));
        }
        let a = load_texture(ctx, "top-avatar-1", include_bytes!("../assets/avatar_sun.png"));
        let b = load_texture(ctx, "top-avatar-2", include_bytes!("../assets/avatar_moon.png"));
        Self {
            sun: a, moon: b,
            one: load_texture(ctx, "onedrive", include_bytes!("../assets/onedrive.png")),
            google: load_texture(ctx, "google-drive", include_bytes!("../assets/google_drive.png")),
            c2s: load_texture(ctx, "c2sdrive", include_bytes!("../assets/c2sdrive.png")),
            partitions,
        }
    }
}

fn load_texture(ctx: &egui::Context, name: &str, data: &[u8]) -> TextureHandle {
    let img = image::load_from_memory(data).unwrap_or_else(|_| {
        image::DynamicImage::new_rgba8(2, 2)
    });
    // 元画像の透過は維持し、角部分だけを円形マスクで徐々に透明化する。
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;
    let radius = (w.min(h) as f32 * 0.5) - 0.5;
    for (x, y, px) in rgba.enumerate_pixels_mut() {
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        let d = (dx * dx + dy * dy).sqrt();
        let mask = (radius - d + 0.5).clamp(0.0, 1.0);
        px[3] = (px[3] as f32 * mask) as u8;
    }
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    ctx.load_texture(name, color, TextureOptions::LINEAR)
}

pub fn install_japanese_font(ctx: &egui::Context) {
    // Windowsのシステムフォントをその場で読み込み、配布物にはフォントを含めない。
    let paths = [
        r"C:\Windows\Fonts\YuGothM.ttc",
        r"C:\Windows\Fonts\YuGothR.ttc",
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\msgothic.ttc",
        r"C:\Windows\Fonts\msyh.ttc",
    ];
    let mut fonts = FontDefinitions::default();
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert("japanese".to_owned(), std::sync::Arc::new(FontData::from_owned(bytes)));
            if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
                prop.insert(0, "japanese".to_owned());
            }
            if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                mono.insert(0, "japanese".to_owned());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

pub struct Dashboard {
    info: DeviceInfo,
    rx: Receiver<Update>,
    images: ImageSet,
    partitions: Vec<Partition>,
    ram_used: u64,
    ram_total: u64,
    loading: bool,
    toast: Option<(String, Instant)>,
}

impl Dashboard {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_japanese_font(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::TRANSPARENT;
        style.visuals.window_fill = Color32::from_rgb(255, 249, 243);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        // ラベルのドラッグ選択（複数ラベルを跨ぐ選択も含む）を無効化する。
        style.interaction.selectable_labels = false;
        style.interaction.multi_widget_text_select = false;
        cc.egui_ctx.set_style(style);
        Self {
            info: system::get_device_info(),
            rx: system::start_monitor(),
            images: ImageSet::new(&cc.egui_ctx),
            partitions: Vec::new(),
            ram_used: 0,
            ram_total: 0,
            loading: true,
            toast: None,
        }
    }

    fn handle_updates(&mut self) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Memory { used, total } => { self.ram_used = used; self.ram_total = total; }
                Update::Partitions(p) => { self.partitions = p; self.loading = false; }
            }
        }
    }

    fn header(&self, ui: &mut Ui) {
        // 1400px基準。SYSTEM OVERVIEWは右端に余白を確保したまま配置。
        let available = ui.available_width();
        let right_padding = 24.0;
        let gutter = 20.0;
        let layout_width = (available - right_padding).max(0.0);
        if available >= 1050.0 {
            let right_w = (layout_width * 0.435).clamp(490.0, 630.0);
            let left_w = (layout_width - right_w - gutter).max(0.0);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, 220.0), egui::Layout::top_down(Align::Min),
                    |ui| { ui.set_width(left_w); self.brand(ui); },
                );
                ui.add_space(gutter - 8.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(right_w, 220.0), egui::Layout::top_down(Align::Min),
                    |ui| { ui.set_width(right_w); self.information(ui); },
                );
            });
        } else {
            // 狭いウィンドウでも水平スクロールさせない。
            self.brand(ui);
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                egui::vec2(layout_width, 220.0), egui::Layout::top_down(Align::Min),
                |ui| { ui.set_width(layout_width); self.information(ui); },
            );
        }
    }

    fn brand(&self, ui: &mut Ui) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            draw_avatar(ui, &self.images.sun, true);
            ui.add_space(10.0);
            draw_avatar(ui, &self.images.moon, false);
            ui.add_space(33.0); // 文字グループ全体を右へ20px移動
            ui.vertical(|ui| {
                ui.add_space(11.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("光と影のソネット").color(BLUE).size(30.0).strong());
                    ui.add_space(3.0);
                    ui.label(RichText::new("(@C2Sonnet)").color(LILAC).size(17.0).strong());
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Chiaroscuro Sonnet").size(21.0).color(INK));
                ui.label(RichText::new("導くひかりと寄り添えカゲ").size(18.0).color(MUTED));
                ui.add_space(14.0);
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(255, 252, 246, 216))
                    .corner_radius(20)
                    .inner_margin(egui::Margin::symmetric(12, 5))
                    .show(ui, |ui| {
                        ui.label(RichText::new("☀  YOUR STORAGE, AT A GLANCE  ☾")
                            .size(14.0).color(MINT).strong());
                    });
            });
        });
    }

    fn information(&self, ui: &mut Ui) {
        let width = ui.available_width();
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(255, 254, 251, 240))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(236, 220, 208)))
            .corner_radius(21)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(width - 24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("SYSTEM OVERVIEW").color(BLUE).size(12.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("● LIVE").color(MINT).size(10.0).strong());
                    });
                });
                ui.add_space(4.0);
                let title = format!("{} @ {}", self.info.hostname, self.info.workgroup);
                info_line(ui, "DEVICE", &title);
                info_line(ui, "DESCRIPTION", &self.info.description);
                info_line(ui, "SYSTEM", &self.info.windows);
                info_line(ui, "USER", &self.info.user);
                info_line(ui, "CPU", &self.info.cpu);
                info_line(ui, "RAM", &format!("{} / {}  ({:.2}% Used)",
                    system::capacity(self.ram_used), system::capacity(self.ram_total),
                    system::usage_ratio(self.ram_used, self.ram_total)));
                let (used, total) = system::disk_totals(&self.partitions);
                info_line(ui, "DISK", &format!("{} / {}  ({:.2}% Used)",
                    system::capacity(used), system::capacity(total),
                    system::usage_ratio(used, total)));
            });
    }

    fn cards(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("パーティション一覧").size(22.0).strong().color(INK));
            ui.add_space(7.0);
            egui::Frame::new().fill(Color32::from_rgb(251, 231, 213))
                .corner_radius(30).inner_margin(egui::Margin::symmetric(12, 5))
                .show(ui, |ui| { ui.label(RichText::new("C: – M:").size(13.5).color(BLUE)); });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("容量情報 30秒ごとに更新").size(11.5).color(MUTED));
            });
        });
        ui.add_space(11.0);
        if self.loading {
            ui.add_space(35.0);
            ui.vertical_centered(|ui| { ui.spinner(); ui.label("パーティションを読み込み中…"); });
            ui.add_space(75.0);
            return;
        }
        if self.partitions.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("C: ～ M: に対象ドライブが見つかりませんでした。").color(MUTED));
            });
            ui.add_space(60.0);
            return;
        }

        // 1400px時は3列。横に溢れないよう、狭いときは2列/1列へ切替。
        let space = ui.available_width();
        let count: usize = if space >= 1130.0 { 3 } else if space >= 750.0 { 2 } else { 1 };
        // eguiのhorizontalは item_spacing が自動挿入されるため、その幅だけ差し引く。
        let gap = 13.0;
        let card_w = ((space - (count - 1) as f32 * gap) / count as f32).max(0.0);
        for group in self.partitions.chunks(count) {
            // 行の高さは説明の折返し・タグの段数から計算し、余白を最小限にする。
            let row_height = group.iter().map(|part| {
                let category = categories::identify(&part.label);
                card_height(ui, part, category, card_w)
            }).fold(0.0_f32, f32::max);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for part in group {
                    let category = categories::identify(&part.label);
                    let texture = category.and_then(|c| self.images.partitions.get(c.name));
                    if draw_partition_card(ui, part, category, texture, card_w, row_height) {
                        let root = format!("{}:\\", part.letter);
                        match std::process::Command::new("explorer.exe").arg(root).spawn() {
                            Ok(_) => self.toast = Some((format!("{}: を開きました", part.letter), Instant::now())),
                            Err(e) => self.toast = Some((format!("ドライブを開けません: {e}"), Instant::now())),
                        }
                    }
                }
            });
            ui.add_space(5.0);
        }
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("クイックアクセス").color(INK).size(19.0).strong());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("フォルダー・クラウドを開く").color(MUTED).size(12.0));
            });
        });
        ui.add_space(10.0);
        let width = ui.available_width();
        // 5つのボタンを固定の1行に収める。gapはここだけ一括設定。
        let gap = 9.0;
        let button_w = ((width - gap * 4.0) / 5.0).max(0.0);
        let actions = [
            ("RAMDisk", 0u8), ("SSDCache", 1u8), ("OneDrive", 2u8),
            ("Google Drive", 3u8), ("C2SDrive", 4u8),
        ];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (name, action) in actions {
                let texture = match action {
                    0 => &self.images.sun,
                    1 => &self.images.moon,
                    2 => &self.images.one,
                    3 => &self.images.google,
                    _ => &self.images.c2s,
                };
                if draw_quick_button(ui, name, texture, button_w) {
                    match open_action(action) {
                        Ok(_) => self.toast = Some((format!("{} を開きました", name), Instant::now())),
                        Err(e) => self.toast = Some((e, Instant::now())),
                    }
                }
            }
        });
        if let Some((message, when)) = &self.toast {
            if when.elapsed() < Duration::from_secs(5) {
                ui.add_space(4.0);
                ui.label(RichText::new(message).color(BLUE).size(12.0));
            }
        }
    }

}

impl eframe::App for Dashboard {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_updates();
        ctx.request_repaint_after(Duration::from_millis(500));

        // 画面の背景を先に全面描画する。パネル同士の境界に背景未描画の黒帯を出さない。
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(ctx.content_rect(), 0.0, Color32::from_rgb(255, 249, 243));

        // 固定の上部ヘッダー。ScrollAreaには含めず、境界の線は描かない。
        egui::TopBottomPanel::top("fixed_system_overview")
            .resizable(false)
            .exact_height(260.0)
            .frame(egui::Frame::new()
                .fill(Color32::from_rgb(255, 249, 241))
                .stroke(Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(21, 6)))
            .show(ctx, |ui| {
                ui.add_space(2.0);
                self.header(ui);
            });

        // 固定のクイックアクセス。すべてのボタンが常に同じ行に残る。
        egui::TopBottomPanel::bottom("fixed_quick_access")
            .resizable(false)
            .exact_height(158.0)
            .frame(egui::Frame::new()
                .fill(Color32::from_rgb(255, 249, 243))
                .stroke(Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(20, 12)))
            .show(ctx, |ui| {
                self.footer(ui);
            });

        // 中央のパーティション一覧のみY方向にスクロール。Xスクロールは禁止。
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(255, 251, 245)).stroke(Stroke::NONE))
            .show(ctx, |ui| {
                paint_gradient(ui);
                egui::ScrollArea::vertical()
                    .id_salt("partition_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(10.0);
                        let content_width = (ui.available_width() - 40.0).max(0.0);
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.set_width(content_width);
                                self.cards(ui);
                                ui.add_space(20.0);
                            });
                        });
                    });
            });
    }
}

fn info_line(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([104.0, 15.0], egui::Label::new(RichText::new(label).color(MUTED).size(10.5).strong()));
        ui.add(egui::Label::new(RichText::new(value).color(INK).size(11.5)).truncate())
            .on_hover_text(value);
    });
}

fn draw_avatar(ui: &mut Ui, texture: &TextureHandle, sun: bool) {
    // 旧版の74px portraitのちょうど1.6倍。サン/月のマークも追随。
    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 146.0), Sense::hover());
    let portrait = Rect::from_center_size(rect.center() + egui::vec2(0.0, 12.0), egui::vec2(118.4, 118.4));
    let p = ui.painter();
    p.circle_filled(portrait.center(), 60.0, Color32::from_rgba_unmultiplied(255, 255, 255, 180));
    p.image(texture.id(), portrait, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), W);
    p.circle_stroke(portrait.center(), 59.2, Stroke::new(1.8_f32, Color32::from_rgb(235, 204, 185)));
    let badge = rect.center_top() + egui::vec2(0.0, 7.0);
    p.circle_filled(badge, 17.0, Color32::from_rgb(255, 251, 243));
    if sun { draw_sun(p, badge); } else { draw_moon(p, badge); }
}

fn draw_sun(p: &egui::Painter, c: Pos2) {
    let color = Color32::from_rgb(255, 162, 65);
    p.circle_stroke(c, 4.2, Stroke::new(1.7_f32, color));
    for n in 0..8 {
        let a = (n as f32) * std::f32::consts::TAU / 8.0;
        let v = egui::vec2(a.cos(), a.sin());
        p.line_segment([c + v * 7.0, c + v * 10.0], Stroke::new(1.5_f32, color));
    }
}

fn draw_moon(p: &egui::Painter, c: Pos2) {
    let color = Color32::from_rgb(112, 103, 213);
    let points: Vec<Pos2> = (0..=28).map(|i| {
        let a = -1.25 + (i as f32 / 28.0) * 3.70;
        c + egui::vec2(a.cos() * 7.2, a.sin() * 7.2)
    }).collect();
    p.add(egui::Shape::line(points, Stroke::new(2.0_f32, color)));
    p.line_segment([c + egui::vec2(2.3, -6.8), c + egui::vec2(0.1, 4.3)], Stroke::new(1.2_f32, color));
}

/// 正確なピクセル領域内だけに描画する。eguiの自動折返しでテキスト幅がゼロにならない。
/// 説明文の改行とタグ折り返し数から必要なカード高さを算出する。
/// 各行では最大高さを共通化するため、カードの下余白が大きくなりすぎない。
fn card_height(ui: &Ui, part: &Partition, category: Option<&Category>, width: f32) -> f32 {
    let text_w = (width - 149.0).max(20.0);
    let desc = if let Some(c) = category { c.description }
        else if part.error { "ドライブが使用できないか、アクセスが拒否されました。" }
        else { "登録済みの分類名を含まないドライブ。用途はボリューム名を確認してください。" };
    let lines = wrap_lines(ui.painter(), desc, &FontId::proportional(15.5), MUTED, text_w, 5);
    let fallback = ["ストレージ", "未分類"];
    let tags: &[&str] = match category { Some(c) => c.tags, None => &fallback };
    let mut rows = 1;
    let mut filled = 0.0_f32;
    for tag in tags.iter().take(5) {
        let tw = ui.painter().layout_no_wrap((*tag).to_owned(), FontId::proportional(14.0), BLUE).size().x + 18.0;
        if filled > 0.0 && filled + tw > text_w {
            rows += 1;
            filled = 0.0;
        }
        filled += tw + 6.0;
    }
    // 説明文とタグを実測して適切なカード高さを確保（下余白は15px）。
    (198.0 + lines.len() as f32 * 23.0 + (rows - 1) as f32 * 29.0)
        .max(255.0)
}

/// 文字・タグの描画はカードの明示的な矩形内に収める。
/// 戻り値: カードをシングルクリックしたかどうか。
fn draw_partition_card(ui: &mut Ui, part: &Partition, category: Option<&Category>, texture: Option<&TextureHandle>, width: f32, height: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter_at(rect);
    let inner = painter.with_clip_rect(rect);
    let fill = if hovered { Color32::from_rgb(255, 246, 235) } else { Color32::from_rgb(255, 254, 252) };
    inner.rect_filled(rect, 20.0, fill);
    inner.rect_stroke(rect, 20.0,
        Stroke::new(if hovered { 2.0_f32 } else { 1.0_f32 },
            if hovered { Color32::from_rgb(205, 144, 121) } else { Color32::from_rgb(237, 220, 208) }),
        StrokeKind::Inside);
    if hovered {
        inner.rect_filled(Rect::from_min_size(rect.min + egui::vec2(8.0, 16.0), egui::vec2(4.0, height - 32.0)), 3.0, BLUE);
    }

    // 旧アイコン60px → 96px (= 1.6倍)。カード全体の縦中央に置く。
    let icon_center = egui::pos2(rect.left() + 66.0, rect.center().y);
    inner.circle_filled(icon_center, 52.0, Color32::from_rgb(255, 241, 231));
    if let Some(texture) = texture {
        inner.image(texture.id(), Rect::from_center_size(icon_center, egui::vec2(96.0, 96.0)),
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), W);
    } else {
        inner.text(icon_center, Align2::CENTER_CENTER, "◉", FontId::proportional(38.0), BLUE);
    }

    let x = rect.left() + 132.0;
    let right = rect.right() - 17.0;
    let text_w = (right - x).max(0.0);
    let local = categories::is_local_badge(&part.label);
    let title_font = FontId::proportional(21.5);
    let title = format!("({}:) {}", part.letter, categories::display_name(&part.label));
    let max_title_w = (text_w - if local { 72.0 } else { 0.0 }).max(10.0);
    let short_title = fit_text(&inner, &title, &title_font, INK, max_title_w);
    let title_rect = inner.text(egui::pos2(x, rect.top() + 32.0), Align2::LEFT_CENTER,
        short_title, title_font, INK);
    if local {
        let badge = Rect::from_min_size(
            egui::pos2((title_rect.right() + 6.0).min(right - 66.0), rect.top() + 18.5),
            egui::vec2(65.0, 27.0));
        inner.rect_filled(badge, 13.5, Color32::from_rgb(220, 239, 224));
        // 縦・横ともバッジの中心に配置。
        inner.text(badge.center(), Align2::CENTER_CENTER, "Local", FontId::proportional(14.0), Color32::from_rgb(40, 110, 77));
    }

    if part.error || part.total == 0 {
        inner.text(egui::pos2(x, rect.top() + 85.0), Align2::LEFT_CENTER,
            "容量を取得できません", FontId::proportional(15.0), MUTED);
    } else {
        let fraction = part.fraction().clamp(0.0, 1.0);
        let bar_color = if fraction >= 0.90 { Color32::from_rgb(151, 105, 163) }
            else if fraction >= 0.75 { Color32::from_rgb(213, 95, 98) }
            else if fraction > 0.50 { Color32::from_rgb(227, 170, 75) }
            else { Color32::from_rgb(102, 164, 167) };
        let total_w = 76.0;
        let bar_w = (text_w - total_w - 8.0).max(20.0);
        let bar = Rect::from_min_size(egui::pos2(x, rect.top() + 72.0), egui::vec2(bar_w, 34.0));
        inner.rect_filled(bar, 12.0, Color32::from_rgb(244, 237, 229));
        if fraction > 0.0 {
            let used_w = (bar.width() * fraction).max(3.0).min(bar.width());
            inner.rect_filled(Rect::from_min_size(bar.min, egui::vec2(used_w, bar.height())), 12.0, bar_color);
        }
        // プログレスバー内の可読性改善: 文字の下に半透明アイボリー背景を置く。
        // 充填率が変わっても文字と背景の明度差を確保する。
        let text_font = FontId::proportional(13.5);
        let used_text = system::short_capacity(part.used());
        let free_text = system::short_capacity(part.free);
        let used_size = inner.layout_no_wrap(used_text.clone(), text_font.clone(), INK).size().x;
        let free_size = inner.layout_no_wrap(free_text.clone(), text_font.clone(), INK).size().x;
        let pill_y = bar.center().y;
        let pill_h = 24.0;
        let left_pill = Rect::from_center_size(
            egui::pos2(bar.left() + 7.0 + (used_size + 12.0) * 0.5, pill_y),
            egui::vec2((used_size + 12.0).min(bar.width() * 0.49), pill_h));
        let right_pill = Rect::from_center_size(
            egui::pos2(bar.right() - 7.0 - (free_size + 12.0) * 0.5, pill_y),
            egui::vec2((free_size + 12.0).min(bar.width() * 0.49), pill_h));
        inner.rect_filled(left_pill, 8.0, Color32::from_rgba_unmultiplied(255, 250, 244, 224));
        inner.rect_filled(right_pill, 8.0, Color32::from_rgba_unmultiplied(255, 250, 244, 224));
        inner.text(left_pill.center(), Align2::CENTER_CENTER, used_text, text_font.clone(), Color32::from_rgb(60, 47, 43));
        inner.text(right_pill.center(), Align2::CENTER_CENTER, free_text, text_font, Color32::from_rgb(60, 47, 43));
        inner.text(egui::pos2(right, bar.center().y), Align2::RIGHT_CENTER,
            system::short_capacity(part.total), FontId::proportional(13.5), INK);
        inner.text(egui::pos2(x, rect.top() + 123.0), Align2::LEFT_CENTER,
            format!("{:.2}% Used", fraction * 100.0), FontId::proportional(14.0), MUTED);
    }

    let desc = if let Some(c) = category { c.description }
        else if part.error { "ドライブが使用できないか、アクセスが拒否されました。" }
        else { "登録済みの分類名を含まないドライブ。用途はボリューム名を確認してください。" };
    // 明示改行(\n)と自動折返しの両方を扱う。3列でも読みやすい文字サイズ。
    let desc_font = FontId::proportional(15.5);
    let description_lines = wrap_lines(&inner, desc, &desc_font, MUTED, text_w, 5);
    let desc_top = rect.top() + 151.0;
    let desc_line_h = 23.0;
    for (line_index, line) in description_lines.iter().enumerate() {
        inner.text(egui::pos2(x, desc_top + line_index as f32 * desc_line_h),
            Align2::LEFT_CENTER, line, desc_font.clone(), MUTED);
    }

    // タグの上下余白は圧縮し、中心揃えの文字を描画する。
    let fallback = ["ストレージ", "未分類"];
    let tags: &[&str] = match category { Some(c) => c.tags, None => &fallback };
    let mut tag_x = x;
    let mut tag_y = desc_top + description_lines.len() as f32 * desc_line_h + 7.0;
    let tag_font = FontId::proportional(14.0);
    for tag in tags.iter().take(5) {
        let tw = inner.layout_no_wrap((*tag).to_owned(), tag_font.clone(), BLUE).size().x + 18.0;
        if tag_x > x && tag_x + tw > right {
            tag_x = x;
            tag_y += 29.0;
        }
        if tag_y + 25.0 > rect.bottom() - 11.0 { break; }
        let pill = Rect::from_min_size(egui::pos2(tag_x, tag_y), egui::vec2(tw.min(text_w), 25.0));
        inner.rect_filled(pill, 12.5, Color32::from_rgb(250, 234, 223));
        inner.text(pill.center(), Align2::CENTER_CENTER, *tag, tag_font.clone(), BLUE);
        tag_x += tw + 6.0;
    }

    // painter描画のため文字をドラッグ選択できず、カードの全面がクリック領域。
    let clicked = response.clicked();
    response.on_hover_text(format!("ラベル: {}\nファイルシステム: {}\n{}\nシングルクリックでドライブを開く", part.label, part.fs, desc));
    clicked
}

/// 実際のフォント幅を測って省略記号を付ける。描画領域外にはみ出さない。
fn fit_text(p: &egui::Painter, text: &str, font: &FontId, color: Color32, max_width: f32) -> String {
    let mut value = text.to_owned();
    if p.layout_no_wrap(value.clone(), font.clone(), color).size().x <= max_width { return value; }
    while !value.is_empty() {
        value.pop();
        let candidate = format!("{}…", value);
        if p.layout_no_wrap(candidate.clone(), font.clone(), color).size().x <= max_width { return candidate; }
    }
    String::new()
}

/// 日本語・英語混在の文章を実測幅で最大行数に折り返す。
fn wrap_lines(p: &egui::Painter, text: &str, font: &FontId, color: Color32, max_width: f32, max_lines: usize) -> Vec<String> {
    if max_lines == 0 { return Vec::new(); }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut truncated = false;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            if lines.len() >= max_lines { truncated = true; break; }
            continue;
        }
        let candidate = format!("{}{}", current, ch);
        if !current.is_empty() && p.layout_no_wrap(candidate, font.clone(), color).size().x > max_width {
            lines.push(std::mem::take(&mut current));
            if lines.len() >= max_lines { truncated = true; break; }
        }
        current.push(ch);
    }
    if !truncated && lines.len() < max_lines && !current.is_empty() { lines.push(current); }
    if truncated {
        if let Some(last) = lines.last_mut() {
            while !last.is_empty() && p.layout_no_wrap(format!("{}…", last), font.clone(), color).size().x > max_width {
                last.pop();
            }
            last.push('…');
        }
    }
    lines
}

fn draw_quick_button(ui: &mut Ui, label: &str, icon: &TextureHandle, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 82.0), Sense::click());
    let hovered = response.hovered();
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 17.0, if hovered { Color32::from_rgb(255, 251, 246) } else { Color32::WHITE });
    p.rect_stroke(rect, 17.0,
        Stroke::new(if hovered { 1.8_f32 } else { 1.0_f32 },
            if hovered { Color32::from_rgb(208, 149, 124) } else { Color32::from_rgb(235, 215, 204) }),
        StrokeKind::Inside);
    // 旧28pxの80% = 22.4px。アイコン左端はボタン左端から正確に10px。
    let font = FontId::proportional(22.4);
    let icon_size = if width < 205.0 { 35.0 } else { 46.0 };
    let icon_x = rect.left() + 10.0;
    let ico = Rect::from_min_size(
        egui::pos2(icon_x, rect.center().y - icon_size * 0.5),
        egui::vec2(icon_size, icon_size));
    p.circle_filled(ico.center(), icon_size * 0.53, Color32::from_rgb(255, 242, 232));
    p.image(icon.id(), ico, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), W);

    // 残りの領域で文字を水平中央揃え。日本語高DPIでも省略表示してはみ出さない。
    let text_left = ico.right() + 8.0;
    let text_right = rect.right() - 8.0;
    let text_width = (text_right - text_left).max(0.0);
    let shown_label = fit_text(&p, label, &font, INK, text_width);
    let text_center = egui::pos2((text_left + text_right) * 0.5, rect.center().y);
    p.text(text_center, Align2::CENTER_CENTER, shown_label, font,
        if hovered { BLUE } else { INK });
    response.on_hover_text("クリックで開く").clicked()
}

fn open_action(id: u8) -> Result<(), String> {
    if id == 4 { return webbrowser::open("https://drive.c2s.me").map_err(|e| format!("ブラウザを開けません: {e}")); }
    let path = match id {
        0 => PathBuf::from(r"A:\"),
        1 => PathBuf::from(r"B:\"),
        2 => find_onedrive().ok_or_else(|| "OneDriveフォルダが見つかりません。同期設定を確認してください。".to_owned())?,
        3 => PathBuf::from(r"Z:\"),
        _ => return Err("不明な操作です".into()),
    };
    if !path.is_dir() { return Err(format!("{} が存在しないか、アクセスできません。", path.display())); }
    std::process::Command::new("explorer.exe")
        .arg(path).spawn().map(|_| ()).map_err(|e| format!("エクスプローラーを起動できません: {e}"))
}

fn find_onedrive() -> Option<PathBuf> {
    for key in ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"] {
        if let Some(path) = std::env::var_os(key) {
            let folder = PathBuf::from(path);
            if folder.is_dir() { return Some(folder); }
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let folder = PathBuf::from(home).join("OneDrive");
        if folder.is_dir() { return Some(folder); }
    }
    None
}

fn paint_gradient(ui: &Ui) {
    let rect = ui.max_rect();
    let p = ui.painter_at(rect);
    // 高明度・低彩度の暖色グラデーション（アイボリー→薄いピーチ）。
    let top = [255.0, 251.0, 245.0];
    let bottom = [250.0, 242.0, 235.0];
    for i in 0..80 {
        let t = i as f32 / 79.0;
        let color = Color32::from_rgb(
            (top[0] * (1.0 - t) + bottom[0] * t) as u8,
            (top[1] * (1.0 - t) + bottom[1] * t) as u8,
            (top[2] * (1.0 - t) + bottom[2] * t) as u8,
        );
        let y0 = rect.top() + rect.height() * (i as f32 / 80.0);
        let y1 = rect.top() + rect.height() * ((i + 1) as f32 / 80.0) + 1.0;
        p.rect_filled(Rect::from_min_max(egui::pos2(rect.left(), y0), egui::pos2(rect.right(), y1)), 0.0, color);
    }
    // うっすら暖色の光を追加。
    for i in (0..=16).rev() {
        let t = i as f32 / 16.0;
        p.circle_filled(rect.left_top() + egui::vec2(110.0, 86.0), 270.0 * t,
            Color32::from_rgba_unmultiplied(255, 220, 171, 2));
        p.circle_filled(rect.right_top() + egui::vec2(-120.0, 190.0), 320.0 * t,
            Color32::from_rgba_unmultiplied(248, 207, 195, 2));
    }
}
