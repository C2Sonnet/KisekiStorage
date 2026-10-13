//! eframe / egui によるWindows GUI。
use std::{collections::HashMap, path::PathBuf, sync::mpsc::{self, Receiver, Sender, TryRecvError}, thread, time::{Duration, Instant}};
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

/// Zen Maru Gothic を実行ファイルに埋め込み、Windows側のフォント環境に依存しない。
/// 同梱の Regular と Bold は SIL OFL 1.1（assets/fonts/OFL.txt）に従う。
fn install_app_font(ctx: &egui::Context) {
    const REGULAR: &str = "zen-maru-regular";
    const BOLD: &str = "zen-maru-bold";
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        REGULAR.to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../assets/fonts/ZenMaruGothic-Regular.ttf"))),
    );
    fonts.font_data.insert(
        BOLD.to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../assets/fonts/ZenMaruGothic-Bold.ttf"))),
    );
    // 通常の日本語・英語UI文字はZen Maru Gothicを優先。
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        proportional.insert(0, REGULAR.to_owned());
    }
    // 数値・等幅UI文字にも日本語のフォールバックを用意。
    if let Some(monospace) = fonts.families.get_mut(&FontFamily::Monospace) {
        monospace.push(REGULAR.to_owned());
    }
    fonts.families.insert(FontFamily::Name("zen-maru-bold".into()), vec![BOLD.to_owned(), REGULAR.to_owned()]);
    ctx.set_fonts(fonts);
}

fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("zen-maru-bold".into()))
}

#[derive(Clone, Copy)]
enum OpenTarget {
    Drive(char),
    Quick(u8),
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
    action_tx: Sender<String>,
    action_rx: Receiver<String>,
    last_action: Option<Instant>,
    monitor_error: bool,
}

impl Dashboard {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_app_font(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::TRANSPARENT;
        style.visuals.window_fill = Color32::from_rgb(255, 249, 243);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        // ラベルのドラッグ選択（複数ラベルを跨ぐ選択も含む）を無効化する。
        style.interaction.selectable_labels = false;
        style.interaction.multi_widget_text_select = false;
        cc.egui_ctx.set_style(style);
        let (action_tx, action_rx) = mpsc::channel();
        Self {
            info: system::get_device_info(),
            rx: system::start_monitor(),
            images: ImageSet::new(&cc.egui_ctx),
            partitions: Vec::new(),
            ram_used: 0,
            ram_total: 0,
            loading: true,
            toast: None,
            action_tx,
            action_rx,
            last_action: None,
            monitor_error: false,
        }
    }

    fn handle_updates(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(Update::Memory { used, total }) => {
                    self.ram_used = used;
                    self.ram_total = total;
                }
                Ok(Update::Partitions(partitions)) => {
                    self.partitions = partitions;
                    self.loading = false;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.monitor_error = true;
                    break;
                }
            }
        }
        while let Ok(message) = self.action_rx.try_recv() {
            self.toast = Some((message, Instant::now()));
        }
    }

    /// 切断されたネットワークドライブ等でGUIの応答を止めないため、
    /// パス確認・Explorer/ブラウザ起動をワーカースレッドで実施する。
    fn queue_open(&mut self, target: OpenTarget) {
        let now = Instant::now();
        if self.last_action.is_some_and(|last| now.duration_since(last) < Duration::from_millis(600)) {
            return; // 連続クリックによる大量起動を防ぐ。
        }
        self.last_action = Some(now);
        let sender = self.action_tx.clone();
        let result = thread::Builder::new().name("kiseki-open".to_owned()).spawn(move || {
            let result = match target {
                OpenTarget::Drive(letter) => {
                    let drive = PathBuf::from(format!("{}:\\", letter));
                    open_directory_in_explorer(&drive)
                }
                OpenTarget::Quick(id) => open_action(id),
            };
            let message = match (target, result) {
                (OpenTarget::Drive(letter), Ok(())) => format!("{}: のエクスプローラーを起動しました", letter),
                (OpenTarget::Quick(_), Ok(())) => "起動要求を送信しました".to_owned(),
                (_, Err(message)) => message,
            };
            let _ = sender.send(message);
        });
        if let Err(error) = result {
            self.toast = Some((format!("起動処理を開始できません: {error}"), now));
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
            ui.label(RichText::new("パーティション一覧").font(bold_font(19.5)).color(INK));
            ui.add_space(7.0);
            egui::Frame::new().fill(Color32::from_rgb(251, 231, 213))
                .corner_radius(30).inner_margin(egui::Margin::symmetric(12, 5))
                .show(ui, |ui| { ui.label(RichText::new("C: – M:").size(12.0).color(BLUE)); });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("容量情報 30秒ごとに更新").size(10.5).color(MUTED));
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
                let message = if self.monitor_error {
                    "ドライブ監視処理が終了しました。アプリを再起動してください。"
                } else { "C: ～ M: に対象ドライブが見つかりませんでした。" };
                ui.label(RichText::new(message).color(MUTED));
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
        let mut selected_drive = None;
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
                        selected_drive = Some(part.letter);
                    }
                }
            });
            ui.add_space(5.0);
        }
        if let Some(letter) = selected_drive { self.queue_open(OpenTarget::Drive(letter)); }
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("クイックアクセス").color(INK).font(bold_font(19.0)));
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
        let mut selected_action = None;
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
                    selected_action = Some(action);
                }
            }
        });
        if let Some(action) = selected_action { self.queue_open(OpenTarget::Quick(action)); }
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
        ctx.request_repaint_after(Duration::from_secs(1));

        // 3つのパネル自身が全面に不透明な背景を描くため、
        // 別レイヤーへの背景の重複描画は不要（旧 egui 互換性エラーも防止）。
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

fn partition_description(part: &Partition, category: Option<&Category>) -> &'static str {
    if let Some(c) = category { c.description }
    else if part.error { "ドライブが使用できないか、アクセスが拒否されました。" }
    else { "登録済みの分類名を含まないドライブ。用途はボリューム名を確認してください。" }
}

fn partition_tags(category: Option<&Category>) -> &'static [&'static str] {
    match category { Some(c) => c.tags, None => &["ストレージ", "未分類"] }
}

/// 正確なピクセル領域内だけに描画する。eguiの自動折返しでテキスト幅がゼロにならない。
/// 説明文の改行とタグ折り返し数から必要なカード高さを算出する。
/// 各行では最大高さを共通化するため、カードの下余白が大きくなりすぎない。
fn card_height(ui: &Ui, part: &Partition, category: Option<&Category>, width: f32) -> f32 {
    let text_w = (width - 154.0).max(20.0);
    let desc = partition_description(part, category);
    let lines = wrap_lines(ui.painter(), desc, &FontId::proportional(12.8), MUTED, text_w, 4);
    let tags = partition_tags(category);
    let mut rows = 1;
    let mut filled = 0.0_f32;
    for tag in tags.iter().take(5) {
        let tw = ui.painter().layout_no_wrap((*tag).to_owned(), FontId::proportional(12.0), BLUE).size().x + 16.0;
        if filled > 0.0 && filled + tw > text_w {
            rows += 1;
            filled = 0.0;
        }
        filled += tw + 6.0;
    }
    (140.0 + lines.len() as f32 * 18.0 + (rows - 1) as f32 * 22.0).max(205.0)
}

fn draw_shadow_text(p: &egui::Painter, pos: Pos2, anchor: Align2, text: impl Into<String>, font: FontId, color: Color32) {
    let text = text.into();
    p.text(pos + egui::vec2(1.0, 1.0), anchor, text.clone(), font.clone(), Color32::from_rgba_unmultiplied(255, 255, 255, 170));
    p.text(pos, anchor, text, font, color);
}

/// 文字・タグの描画はカードの明示的な矩形内に収める。
/// 戻り値: カードをシングルクリックしたかどうか。
fn draw_partition_card(ui: &mut Ui, part: &Partition, category: Option<&Category>, texture: Option<&TextureHandle>, width: f32, height: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter_at(rect);
    let inner = painter.with_clip_rect(rect);
    let fill = if hovered { Color32::from_rgb(255, 247, 239) } else { Color32::from_rgb(255, 253, 250) };
    inner.rect_filled(rect, 20.0, fill);
    inner.rect_stroke(rect, 20.0,
        Stroke::new(if hovered { 1.8_f32 } else { 1.0_f32 },
            if hovered { Color32::from_rgb(205, 144, 121) } else { Color32::from_rgb(237, 220, 208) }),
        StrokeKind::Inside);

    let icon_center = egui::pos2(rect.left() + 66.0, rect.center().y);
    inner.circle_filled(icon_center, 52.0, Color32::from_rgb(255, 241, 231));
    if let Some(texture) = texture {
        inner.image(texture.id(), Rect::from_center_size(icon_center, egui::vec2(96.0, 96.0)),
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)), W);
    } else {
        inner.text(icon_center, Align2::CENTER_CENTER, "◉", FontId::proportional(38.0), BLUE);
    }

    let x = rect.left() + 132.0;
    let right = rect.right() - 8.0;
    let text_w = (right - x).max(0.0);
    let local = categories::is_local_badge(&part.label);
    let total_text = if !part.error && part.total > 0 { system::short_capacity(part.total) } else { "--".to_owned() };
    let total_font = FontId::proportional(11.5);
    let total_size = inner.layout_no_wrap(total_text.clone(), total_font.clone(), INK).size().x;
    let title_font = bold_font(18.4);
    let title = format!("({}:) {}", part.letter, categories::display_name(&part.label));
    let badge_w = if local { 58.0 } else { 0.0 };
    let max_title_w = (text_w - total_size - badge_w - 18.0).max(10.0);
    let short_title = fit_text(&inner, &title, &title_font, INK, max_title_w);
    let title_rect = inner.text(egui::pos2(x, rect.top() + 28.0), Align2::LEFT_CENTER, short_title, title_font, INK);
    if local {
        let badge = Rect::from_min_size(
            egui::pos2((title_rect.right() + 6.0).min(right - total_size - 68.0), rect.top() + 16.0),
            egui::vec2(54.0, 24.0));
        inner.rect_filled(badge, 12.0, Color32::from_rgb(220, 239, 224));
        inner.text(badge.center(), Align2::CENTER_CENTER, "Local", FontId::proportional(12.0), Color32::from_rgb(40, 110, 77));
    }
    draw_shadow_text(&inner, egui::pos2(rect.right() - 8.0, rect.top() + 28.0), Align2::RIGHT_CENTER, total_text, total_font, INK);

    if part.error || part.total == 0 {
        inner.text(egui::pos2(x, rect.top() + 82.0), Align2::LEFT_CENTER,
            "容量を取得できません", FontId::proportional(13.0), MUTED);
    } else {
        let fraction = part.fraction().clamp(0.0, 1.0);
        let bar_color = if fraction >= 0.90 { Color32::from_rgb(190, 150, 198) }
            else if fraction >= 0.75 { Color32::from_rgb(224, 149, 151) }
            else if fraction > 0.50 { Color32::from_rgb(233, 191, 120) }
            else { Color32::from_rgb(149, 197, 199) };
        let bar = Rect::from_min_size(egui::pos2(x, rect.top() + 58.0), egui::vec2(text_w, 28.0));
        inner.rect_filled(bar, 14.0, Color32::from_rgb(243, 237, 230));
        if fraction > 0.0 {
            let used_w = (bar.width() * fraction).clamp(8.0, bar.width());
            let fill_rect = Rect::from_min_max(bar.min, egui::pos2(bar.left() + used_w, bar.bottom()));
            let clipped = inner.with_clip_rect(bar);
            clipped.rect_filled(fill_rect, 14.0, bar_color);
        }
        let value_font = FontId::proportional(11.4);
        draw_shadow_text(&inner, egui::pos2(bar.left() + 12.0, bar.center().y), Align2::LEFT_CENTER,
            system::short_capacity(part.used()), value_font.clone(), Color32::from_rgb(72, 57, 53));
        draw_shadow_text(&inner, egui::pos2(bar.right() - 12.0, bar.center().y), Align2::RIGHT_CENTER,
            system::short_capacity(part.free), value_font.clone(), Color32::from_rgb(72, 57, 53));
        inner.text(bar.center(), Align2::CENTER_CENTER,
            format!("{:.2}%", fraction * 100.0), FontId::proportional(12.0), Color32::from_rgb(110, 95, 89));
    }

    let desc = partition_description(part, category);
    let desc_font = FontId::proportional(12.8);
    let description_lines = wrap_lines(&inner, desc, &desc_font, MUTED, text_w, 4);
    let desc_top = rect.top() + 112.0;
    let desc_line_h = 18.0;
    for (line_index, line) in description_lines.iter().enumerate() {
        inner.text(egui::pos2(x, desc_top + line_index as f32 * desc_line_h),
            Align2::LEFT_CENTER, line, desc_font.clone(), MUTED);
    }

    let tags = partition_tags(category);
    let mut tag_x = x;
    let mut tag_y = desc_top + description_lines.len() as f32 * desc_line_h + 5.0;
    let tag_font = FontId::proportional(12.0);
    for tag in tags.iter().take(5) {
        let tw = inner.layout_no_wrap((*tag).to_owned(), tag_font.clone(), BLUE).size().x + 16.0;
        if tag_x > x && tag_x + tw > right {
            tag_x = x;
            tag_y += 22.0;
        }
        if tag_y + 20.0 > rect.bottom() - 10.0 { break; }
        let pill = Rect::from_min_size(egui::pos2(tag_x, tag_y), egui::vec2(tw.min(text_w), 20.0));
        inner.rect_filled(pill, 10.0, Color32::from_rgb(250, 234, 223));
        inner.text(pill.center(), Align2::CENTER_CENTER, *tag, tag_font.clone(), BLUE);
        tag_x += tw + 6.0;
    }

    let clicked = response.clicked();
    response.on_hover_text(format!("ラベル: {}
ファイルシステム: {}
{}
シングルクリックでドライブを開く", part.label, part.fs, desc));
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
    open_directory_in_explorer(&path)
}

/// PATH検索で別の explorer.exe を管理者権限で起動しないように、
/// Windows API からWindowsディレクトリを取得し、フルパスを使用する。
fn windows_explorer_path() -> Result<PathBuf, String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetWindowsDirectoryW(buffer: *mut u16, size: u32) -> u32;
    }
    let mut buffer = [0u16; 32768];
    let count = unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if count == 0 || count >= buffer.len() {
        return Err("Windowsディレクトリを確認できません。".to_owned());
    }
    let windows = String::from_utf16(&buffer[..count])
        .map_err(|_| "Windowsディレクトリ名が不正です。".to_owned())?;
    let explorer = PathBuf::from(windows).join("explorer.exe");
    if !explorer.is_file() {
        return Err("Windowsエクスプローラーが見つかりません。".to_owned());
    }
    Ok(explorer)
}

fn open_directory_in_explorer(path: &std::path::Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("{} が存在しないか、アクセスできません。", path.display()));
    }
    // shell/cmd.exeを経由しない。フォルダパスを1つの引数として直接渡す。
    std::process::Command::new(windows_explorer_path()?)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("エクスプローラーを起動できません: {e}"))
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
