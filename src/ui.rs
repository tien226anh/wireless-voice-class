use std::sync::atomic::Ordering;

use eframe::egui::{self, Align, Color32, FontFamily, FontId, RichText, Stroke, Vec2};

use crate::{AppStatus, PaApp, audio::load_f32, i18n::Language, preset::PresetKind};

const BACKGROUND: Color32 = Color32::from_rgb(244, 247, 252);
const INK: Color32 = Color32::from_rgb(28, 39, 62);
const MUTED: Color32 = Color32::from_rgb(98, 112, 135);
const PRIMARY: Color32 = Color32::from_rgb(76, 73, 218);
const TINT: Color32 = Color32::from_rgb(237, 237, 255);
const BORDER: Color32 = Color32::from_rgb(220, 227, 238);
const GREEN: Color32 = Color32::from_rgb(20, 125, 98);
const RED: Color32 = Color32::from_rgb(188, 53, 72);

fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name("Noto Bold".into())))
}

pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Noto Sans".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "Noto Bold".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Bold.ttf")).into(),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Noto Sans".into());
    fonts.families.insert(
        FontFamily::Name("Noto Bold".into()),
        vec!["Noto Bold".into()],
    );
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.extreme_bg_color = Color32::from_rgb(230, 235, 244);
    style.visuals.faint_bg_color = TINT;
    style.visuals.selection.bg_fill = PRIMARY;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = BACKGROUND;
    style.visuals.widgets.inactive.weak_bg_fill = BACKGROUND;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.bg_fill = TINT;
    style.visuals.widgets.hovered.weak_bg_fill = TINT;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, PRIMARY);
    style.visuals.widgets.active.bg_fill = TINT;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, PRIMARY);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 9.into();
    }
    style.spacing.item_spacing = egui::vec2(12.0, 6.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style.spacing.interact_size = egui::vec2(44.0, 36.0);
    style.spacing.slider_width = 230.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(24.0, FontFamily::Name("Noto Bold".into())),
    );
    ctx.set_style(style);
}

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(16)
        .inner_margin(18)
}

fn microphone_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 12, PRIMARY);
    let center = rect.center();
    let scale = size / 48.0;
    painter.rect_filled(
        egui::Rect::from_center_size(
            center + egui::vec2(0.0, -4.0) * scale,
            egui::vec2(10.0, 19.0) * scale,
        ),
        5,
        Color32::WHITE,
    );
    let points = [
        (-9.0, -2.0),
        (-9.0, 4.0),
        (-6.0, 9.0),
        (0.0, 11.0),
        (6.0, 9.0),
        (9.0, 4.0),
        (9.0, -2.0),
    ]
    .map(|(x, y)| center + egui::vec2(x, y) * scale)
    .to_vec();
    painter.add(egui::Shape::line(
        points,
        Stroke::new(2.0 * scale, Color32::WHITE),
    ));
    painter.line_segment(
        [
            center + egui::vec2(0.0, 11.0) * scale,
            center + egui::vec2(0.0, 16.0) * scale,
        ],
        Stroke::new(2.0 * scale, Color32::WHITE),
    );
}

fn step_title(ui: &mut egui::Ui, number: &str, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 15.0, TINT);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            number,
            FontId::proportional(15.0),
            PRIMARY,
        );
        ui.label(bold(title, 19.0));
    });
    ui.add_space(4.0);
}

fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    hint: &str,
) {
    ui.label(label).on_hover_text(hint);
    ui.add(egui::Slider::new(value, range)).on_hover_text(hint);
}

fn device_name(name: &str, language: Language) -> String {
    match name {
        "default" | "pulse" | "pipewire" => format!(
            "{} ({name})",
            language.text("System default", "Mặc định hệ thống")
        ),
        _ => name.to_owned(),
    }
}

impl PaApp {
    fn choose_language(&mut self, language: Language, frame: &mut eframe::Frame) {
        self.language = Some(language);
        if let Some(storage) = frame.storage_mut() {
            language.save(storage);
            storage.flush();
        }
    }

    fn welcome(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(((ui.available_height() - 470.0) / 2.0).max(28.0));
                ui.vertical_centered(|ui| {
                    ui.set_max_width(540.0);
                    microphone_mark(ui, 64.0);
                    ui.add_space(12.0);
                    ui.label(bold("Wireless PA", 32.0));
                    ui.label(RichText::new("Welcome / Xin chào").size(18.0).color(MUTED));
                    ui.add_space(24.0);
                    card().show(ui, |ui| {
                        ui.set_width(490.0);
                        ui.vertical_centered(|ui| {
                            ui.label(bold("Choose your language", 22.0));
                            ui.label(RichText::new("Chọn ngôn ngữ của bạn").color(MUTED));
                            ui.add_space(14.0);
                            ui.columns(2, |columns| {
                                for (column, language) in columns.iter_mut().zip(Language::ALL) {
                                    let button = egui::Button::new(bold(language.name(), 20.0))
                                        .fill(TINT)
                                        .stroke(Stroke::new(1.0_f32, BORDER))
                                        .corner_radius(12);
                                    if column
                                        .add_sized([column.available_width(), 72.0], button)
                                        .clicked()
                                    {
                                        self.choose_language(language, frame);
                                    }
                                }
                            });
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("You can change this anytime in the top bar.")
                                    .size(12.0)
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(
                                    "Bạn có thể đổi ngôn ngữ trên thanh phía trên bất cứ lúc nào.",
                                )
                                .size(12.0)
                                .color(MUTED),
                            );
                        });
                    });
                });
            });
        });
    }

    fn header(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame, l: Language) {
        egui::TopBottomPanel::top("header").frame(egui::Frame::new().fill(Color32::WHITE)
            .inner_margin(egui::Margin::symmetric(24, 16))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                microphone_mark(ui, 44.0);
                let compact = ui.available_width() < 600.0;
                ui.vertical(|ui| {
                    ui.label(bold("Wireless PA", 23.0));
                    if !compact { ui.label(RichText::new(l.text("Your microphone, through your speakers.",
                        "Phát giọng nói từ micrô ra loa.")).size(12.0).color(MUTED));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let mut selected = l;
                    egui::ComboBox::from_id_salt("language").width(130.0)
                        .selected_text(l.name()).show_ui(ui, |ui| {
                        for language in Language::ALL {
                            ui.selectable_value(&mut selected, language, language.name());
                        }
                    }).response.on_hover_text(l.text("Change language. Your sound settings stay the same.",
                        "Đổi ngôn ngữ. Các thiết lập âm thanh được giữ nguyên."));
                    if selected != l { self.choose_language(selected, frame); }
                    if ui.button(l.text("Quick help", "Hướng dẫn")).on_hover_text(l.text(
                        "1. Connect your microphone and speaker.\n2. Select them below.\n3. Start with low speaker volume, then press Start microphone.",
                        "1. Kết nối micrô và loa.\n2. Chọn thiết bị bên dưới.\n3. Giảm âm lượng loa, rồi nhấn Bắt đầu phát micrô.")).clicked() {
                        self.show_help = !self.show_help;
                    }
                });
            });
        });
    }

    fn help(&mut self, ui: &mut egui::Ui, l: Language) {
        egui::Frame::new()
            .fill(TINT)
            .corner_radius(14)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(bold(
                        l.text("Ready in three steps", "Sẵn sàng chỉ với ba bước"),
                        17.0,
                    ));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(l.text("Hide", "Ẩn")).clicked() {
                            self.show_help = false;
                        }
                    });
                });
                ui.label(l.text(
                    "1. Pair your Bluetooth headset in your computer's sound settings.",
                    "1. Ghép đôi tai nghe Bluetooth trong phần cài đặt âm thanh của máy tính.",
                ));
                ui.label(l.text(
                    "2. Choose your microphone and the speaker you want people to hear.",
                    "2. Chọn micrô và loa mà bạn muốn phát âm thanh.",
                ));
                ui.label(l.text(
                    "3. Keep speaker volume low, press Start microphone, and speak.",
                    "3. Để âm lượng loa ở mức thấp, nhấn Bắt đầu phát micrô và nói thử.",
                ));
            });
    }

    fn devices(&mut self, ui: &mut egui::Ui, l: Language) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            step_title(ui, "1", l.text("Connect your devices", "Chọn thiết bị của bạn"));
            ui.add_enabled_ui(self.engine.is_none(), |ui| {
                let input = |ui: &mut egui::Ui, app: &mut Self| {
                    ui.label(bold(l.text("Microphone", "Micrô"), 14.0));
                    egui::ComboBox::from_id_salt("input").width(ui.available_width())
                        .selected_text(app.inputs.get(app.selected_input).map(|d| device_name(&d.name, l))
                            .unwrap_or_else(|| l.text("No microphone found", "Chưa tìm thấy micrô").to_owned()))
                        .show_ui(ui, |ui| {
                        for (index, device) in app.inputs.iter().enumerate() {
                            ui.selectable_value(&mut app.selected_input, index, device_name(&device.name, l));
                        }
                    }).response.on_hover_text(l.text("Choose the headset or microphone you will speak into. For System default, select the device in your computer's sound settings.",
                        "Chọn tai nghe hoặc micrô bạn sẽ dùng để nói. Với Mặc định hệ thống, hãy chọn thiết bị trong cài đặt âm thanh của máy tính."));
                };
                let output = |ui: &mut egui::Ui, app: &mut Self| {
                    ui.label(bold(l.text("Speaker", "Loa"), 14.0));
                    egui::ComboBox::from_id_salt("output").width(ui.available_width())
                        .selected_text(app.outputs.get(app.selected_output).map(|d| device_name(&d.name, l))
                            .unwrap_or_else(|| l.text("No speaker found", "Chưa tìm thấy loa").to_owned()))
                        .show_ui(ui, |ui| {
                        for (index, device) in app.outputs.iter().enumerate() {
                            ui.selectable_value(&mut app.selected_output, index, device_name(&device.name, l));
                        }
                    }).response.on_hover_text(l.text("Choose the speaker for your audience. Keep it away from the microphone to reduce whistling.",
                        "Chọn loa để người nghe nghe được giọng nói. Đặt loa xa micrô để giảm tiếng hú."));
                };
                if ui.available_width() >= 640.0 {
                    ui.columns(2, |columns| { input(&mut columns[0], self); output(&mut columns[1], self); });
                } else { input(ui, self); output(ui, self); }
                ui.add_space(2.0);
                if ui.button(l.text("Refresh devices", "Tìm lại thiết bị")).on_hover_text(l.text(
                    "Use this after connecting or pairing a new device.", "Nhấn sau khi kết nối hoặc ghép đôi thiết bị mới.")).clicked() {
                    self.refresh_devices();
                }
            });
            if self.engine.is_some() {
                ui.label(RichText::new(l.text("Stop playback to change devices.", "Dừng phát để đổi thiết bị.")).size(12.0).color(MUTED));
            } else if self.inputs.is_empty() || self.outputs.is_empty() {
                ui.colored_label(RED, l.text("Connect a microphone and speaker, then refresh the list.",
                    "Hãy kết nối micrô và loa, sau đó tìm lại thiết bị."));
            }
        });
    }

    fn profiles(&mut self, ui: &mut egui::Ui, l: Language) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            step_title(ui, "2", l.text("Choose a sound profile", "Chọn chế độ âm thanh"));
            let mut selected = self.preset;
            let choices = PresetKind::ALL;
            let mut option = |ui: &mut egui::Ui, kind: PresetKind| {
                let active = selected == kind;
                let button = egui::Button::new(bold(kind.label(l), 14.0).color(if active { PRIMARY } else { INK }))
                    .fill(if active { TINT } else { BACKGROUND })
                    .stroke(Stroke::new(if active { 1.5_f32 } else { 1.0_f32 }, if active { PRIMARY } else { BORDER }))
                    .corner_radius(10);
                if ui.add_sized([ui.available_width(), 48.0], button).on_hover_text(format!("{}\n\n{}",
                    kind.description(l), l.text("Changing profiles resets the sound settings and briefly restarts audio if it is running.",
                        "Đổi chế độ sẽ đặt lại thiết lập âm thanh và khởi động lại luồng âm thanh trong giây lát nếu đang phát."))).clicked() {
                    selected = kind;
                }
            };
            if ui.available_width() >= 640.0 {
                ui.columns(3, |columns| {
                    for (column, kind) in columns.iter_mut().zip(choices) { option(column, kind); }
                });
            } else { for kind in choices { option(ui, kind); } }
            if selected != self.preset { self.apply_preset(selected); }
            ui.label(RichText::new(self.preset.description(l)).color(MUTED).size(13.0));
        });
    }

    fn volume(&mut self, ui: &mut egui::Ui, l: Language, peak: f32) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            step_title(ui, "3", l.text("Make yourself heard", "Điều chỉnh giọng nói"));
            ui.label(bold(l.text("Voice volume", "Âm lượng giọng nói"), 14.0));
            ui.spacing_mut().slider_width = (ui.available_width() - 110.0).max(180.0);
            ui.add(egui::Slider::new(&mut self.gain, 0.0..=4.0)
                .custom_formatter(|value, _| format!("{:.0}%", value * 100.0))
                .custom_parser(|text| text.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)))
                .on_hover_text(l.text("Start gently and increase if your voice is too quiet. 100% is the original processed volume; 0% is silent.",
                    "Tăng từ từ nếu giọng nói còn nhỏ. 100% là âm lượng sau xử lý ban đầu; 0% là tắt tiếng."));
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.aec_enabled, l.text("Reduce echo", "Giảm tiếng vọng")).on_hover_text(l.text(
                    "Helps remove speaker sound picked up again by the microphone. Leave this on to start.",
                    "Giúp loại bớt âm thanh từ loa bị micrô thu lại. Nên bật khi bắt đầu sử dụng."));
                ui.checkbox(&mut self.feedback_enabled, l.text("Reduce whistling", "Giảm tiếng hú")).on_hover_text(l.text(
                    "Reduces persistent ringing tones. Speaker placement and low volume still matter.",
                    "Giảm tiếng rít kéo dài. Vẫn cần đặt loa hợp lý và giữ âm lượng vừa phải."));
            });
            let level = if self.engine.is_some() { peak.clamp(0.0, 1.0) } else { 0.0 };
            ui.label(RichText::new(l.text("Voice level", "Mức giọng nói")).size(12.0).color(MUTED));
            ui.add(egui::ProgressBar::new(level).desired_height(10.0).fill(GREEN))
                .on_hover_text(l.text("Shows your voice after sound processing. It moves when you speak while playback is running.",
                    "Hiển thị giọng nói sau xử lý. Thanh sẽ thay đổi khi bạn nói trong lúc đang phát."));
            ui.label(RichText::new(if self.engine.is_some() {
                l.text("Speak into your microphone and watch the level.", "Nói vào micrô và quan sát mức âm thanh.")
            } else { l.text("Press Start microphone below when you're ready.", "Nhấn Bắt đầu phát micrô bên dưới khi bạn đã sẵn sàng.") }).size(12.0).color(MUTED));
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui, l: Language) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::CollapsingHeader::new(bold(l.text("Advanced sound settings", "Thiết lập âm thanh nâng cao"), 16.0))
                .id_salt("advanced").show(ui, |ui| {
                ui.label(RichText::new(l.text("The selected profile is a good starting point. Change these only if you need to fine-tune the sound.",
                    "Chế độ đã chọn là điểm khởi đầu phù hợp. Chỉ thay đổi các mục này khi bạn cần tinh chỉnh âm thanh.")).color(MUTED));
                egui::CollapsingHeader::new(l.text("Voice clarity", "Độ rõ của giọng nói")).id_salt("clarity").show(ui, |ui| {
                    slider(ui, &mut self.high_pass_hz, 40.0..=220.0, l.text("Remove low rumble (Hz)", "Lọc tiếng ù trầm (Hz)"),
                        l.text("Raise this to reduce handling noise. Lower it if your voice sounds thin.", "Tăng để giảm tiếng ù khi cầm micrô. Giảm nếu giọng nói bị mỏng."));
                    slider(ui, &mut self.gate, 0.0..=0.05, l.text("Background noise threshold", "Ngưỡng lọc tiếng ồn nền"),
                        l.text("Raise to mute quiet background noise; lower if quiet words disappear.", "Tăng để tắt bớt tiếng ồn nhỏ; giảm nếu những từ nói nhỏ bị mất."));
                    slider(ui, &mut self.compressor_threshold_db, -40.0..=-3.0, l.text("Compression threshold (dB)", "Ngưỡng nén âm lượng (dB)"),
                        l.text("A more negative value evens out more of your voice's loud peaks.", "Giá trị âm lớn hơn giúp làm đều nhiều đoạn âm thanh lớn hơn."));
                    slider(ui, &mut self.compressor_ratio, 1.0..=10.0, l.text("Compression strength", "Mức nén âm lượng"),
                        l.text("Higher values reduce loud phrases more. 1 means no compression.", "Giá trị cao làm giảm các đoạn nói lớn mạnh hơn. 1 là không nén."));
                    slider(ui, &mut self.compressor_attack_ms, 1.0..=80.0, l.text("Compression attack (ms)", "Thời gian bắt đầu nén (ms)"),
                        l.text("How quickly loud sounds are reduced.", "Tốc độ giảm những âm thanh lớn."));
                    slider(ui, &mut self.compressor_release_ms, 30.0..=600.0, l.text("Compression release (ms)", "Thời gian nhả nén (ms)"),
                        l.text("How quickly normal volume returns after a loud phrase.", "Thời gian âm lượng trở lại bình thường sau một đoạn nói lớn."));
                    slider(ui, &mut self.limit, 0.2..=1.0, l.text("Peak volume limit", "Giới hạn âm lượng đỉnh"),
                        l.text("Limits loud peaks. It cannot repair distortion already present at the microphone.", "Hạn chế âm thanh quá lớn. Không sửa được âm thanh đã bị méo tại micrô."));
                });
                egui::CollapsingHeader::new(l.text("Feedback fine-tuning", "Tinh chỉnh chống hú")).id_salt("feedback").show(ui, |ui| {
                    slider(ui, &mut self.feedback_min_hz, 80.0..=3000.0, l.text("Lowest frequency (Hz)", "Tần số thấp nhất (Hz)"),
                        l.text("Lowest frequency to check for ringing. Keep below the highest frequency.", "Tần số thấp nhất cần kiểm tra tiếng hú. Giữ nhỏ hơn tần số cao nhất."));
                    slider(ui, &mut self.feedback_max_hz, 500.0..=10000.0, l.text("Highest frequency (Hz)", "Tần số cao nhất (Hz)"),
                        l.text("Highest frequency to check for ringing.", "Tần số cao nhất cần kiểm tra tiếng hú."));
                    slider(ui, &mut self.feedback_tonal_ratio, 4.0..=30.0, l.text("Tone detection threshold", "Ngưỡng nhận diện tiếng hú"),
                        l.text("Lower values detect ringing more readily, but may affect wanted sounds.", "Giá trị thấp dễ nhận diện tiếng hú hơn nhưng có thể ảnh hưởng âm thanh cần giữ."));
                    ui.label(l.text("Detection confirmations", "Số lần xác nhận tiếng hú"));
                    ui.add(egui::Slider::new(&mut self.feedback_required_hits, 1..=8)).on_hover_text(l.text(
                        "Higher values wait longer before suppressing a tone.", "Giá trị cao chờ lâu hơn trước khi giảm một âm sắc nghi là tiếng hú."));
                    slider(ui, &mut self.feedback_notch_q, 4.0..=40.0, l.text("Filter focus (Q)", "Độ hẹp của bộ lọc (Q)"),
                        l.text("Higher values reduce a narrower band of sound.", "Giá trị cao làm giảm một dải âm thanh hẹp hơn."));
                    slider(ui, &mut self.feedback_release_seconds, 0.5..=10.0, l.text("Hold suppression (seconds)", "Thời gian giữ chống hú (giây)"),
                        l.text("How long the filter remains after detecting a tone.", "Thời gian giữ bộ lọc sau khi phát hiện tiếng hú."));
                });
                egui::CollapsingHeader::new(l.text("Wireless stability & delay", "Độ ổn định và độ trễ không dây")).id_salt("buffer").show(ui, |ui| {
                    slider(ui, &mut self.adaptive_target_buffer_ms, 10.0..=180.0, l.text("Audio reserve (ms)", "Bộ đệm âm thanh (ms)"),
                        l.text("Increase if sound breaks up. Higher values also add delay. Bluetooth starts at 90 ms.", "Tăng nếu âm thanh bị ngắt quãng. Giá trị cao cũng làm tăng độ trễ. Bluetooth mặc định là 90 ms."));
                    ui.label(l.text("Clock correction strength", "Mức điều chỉnh đồng hồ âm thanh"));
                    ui.add(egui::Slider::new(&mut self.adaptive_correction_strength, 0.0005..=0.01).logarithmic(true))
                        .on_hover_text(l.text("Keep the profile value unless investigating device clock drift.", "Giữ giá trị của chế độ trừ khi đang kiểm tra sai lệch đồng hồ giữa các thiết bị."));
                    ui.label(l.text("Maximum rate correction", "Mức điều chỉnh tốc độ tối đa"));
                    ui.add(egui::Slider::new(&mut self.adaptive_max_correction, 0.003..=0.05).logarithmic(true))
                        .on_hover_text(l.text("Limits resampling changes. 0.025 means 2.5%.", "Giới hạn mức thay đổi tốc độ lấy mẫu. 0,025 tương đương 2,5%."));
                });
                egui::CollapsingHeader::new(l.text("Technical details", "Thông tin kỹ thuật")).id_salt("diagnostics").show(ui, |ui| {
                    if let Some(engine) = &self.engine {
                        ui.label(format!("{}: {} Hz / {} {}", l.text("Input", "Đầu vào"), engine.input_rate, engine.input_channels, l.text("channels", "kênh")));
                        ui.label(format!("{}: {} Hz / {} {}", l.text("Output", "Đầu ra"), engine.output_rate, engine.output_channels, l.text("channels", "kênh")));
                    }
                    ui.label(format!("{}: {} ms", l.text("Echo cancellation latency", "Độ trễ khử tiếng vọng"),
                        if self.engine.is_some() && self.aec_enabled { self.stats.aec_latency_ms.load(Ordering::Relaxed) } else { 0 }));
                    ui.label(format!("{}: {} / {} / {} ms", l.text("AEC delay / search / tail", "AEC: trễ / tìm kiếm / đuôi vọng"),
                        self.aec_preset.max_echo_delay_ms, self.aec_preset.max_search_delay_ms, self.aec_preset.tail_ms));
                    ui.label(format!("{}: {} / {}", l.text("Queue underruns / overruns", "Số lần thiếu / tràn bộ đệm"),
                        self.stats.output_underruns.load(Ordering::Relaxed), self.stats.input_overruns.load(Ordering::Relaxed)));
                    let fill = if self.engine.is_some() { self.stats.queue_fill_permille.load(Ordering::Relaxed) as f32 / 1000.0 } else { 0.0 };
                    ui.label(l.text("Buffer occupancy", "Mức sử dụng bộ đệm"));
                    ui.add(egui::ProgressBar::new(fill.clamp(0.0, 1.0)).show_percentage());
                    let frequency = load_f32(&self.stats.feedback_hz_bits);
                    if self.engine.is_some() && self.feedback_enabled && frequency > 0.0 {
                        ui.label(format!("{}: {frequency:.0} Hz", l.text("Active feedback filter", "Bộ lọc chống hú đang hoạt động")));
                    }
                    if let AppStatus::Failed(error) = &self.status { ui.label(error); }
                });
            });
        });
    }

    fn footer(&mut self, ctx: &egui::Context, l: Language) {
        egui::TopBottomPanel::bottom("playback").frame(egui::Frame::new().fill(Color32::WHITE)
            .inner_margin(egui::Margin::symmetric(24, 18))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let running = self.engine.is_some();
                let available = !self.inputs.is_empty() && !self.outputs.is_empty();
                let error = matches!(self.status, AppStatus::Failed(_));
                let status = if running { l.text("You're live", "Đang phát giọng nói") }
                    else if error { l.text("Couldn't start audio", "Chưa thể phát âm thanh") }
                    else if !available { l.text("Connect your devices", "Hãy kết nối thiết bị") }
                    else { l.text("Ready when you are", "Sẵn sàng khi bạn muốn") };
                let hint = if running { l.text("Your microphone is playing through the speaker.", "Giọng nói từ micrô đang được phát ra loa.") }
                    else if error { l.text("Check your devices and microphone permissions. Details are in Advanced settings.", "Kiểm tra thiết bị và quyền truy cập micrô. Xem chi tiết trong Thiết lập nâng cao.") }
                    else { l.text("Start with a low speaker volume.", "Bắt đầu với âm lượng loa ở mức thấp.") };
                let status_width = (ui.available_width() - 240.0).max(160.0);
                ui.allocate_ui_with_layout(egui::vec2(status_width, 58.0),
                    egui::Layout::top_down(Align::Min), |ui| {
                    ui.set_min_width(status_width);
                    ui.label(bold(status, 17.0).color(if running { GREEN } else if error { RED } else { INK }));
                    ui.label(RichText::new(hint).size(12.0).color(MUTED));
                });
                let button = egui::Button::new(bold(if running { l.text("Stop microphone", "Dừng phát micrô") }
                    else { l.text("Start microphone", "Bắt đầu phát micrô") }, 16.0).color(Color32::WHITE))
                    .fill(if running { RED } else { PRIMARY }).corner_radius(12);
                let response = ui.add_enabled_ui(running || available, |ui| ui.add_sized([224.0, 54.0], button)).inner;
                let clicked = response.on_hover_text(if running {
                    l.text("Stop sending microphone audio to the speaker. You can then change devices.", "Dừng phát âm thanh từ micrô ra loa. Sau đó bạn có thể đổi thiết bị.")
                } else { l.text("Select your microphone and speaker, keep speaker volume low, then click to start speaking.",
                    "Chọn micrô và loa, để âm lượng loa ở mức thấp, rồi nhấn để bắt đầu nói.") })
                    .on_disabled_hover_text(l.text("Connect a microphone and speaker, then click Refresh devices.",
                        "Kết nối micrô và loa, rồi nhấn Tìm lại thiết bị.")).clicked();
                if clicked { if running { self.stop(); } else { self.start(); } }
            });
        });
    }
}

impl eframe::App for PaApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Some(language) = self.language else {
            self.welcome(ctx, frame);
            return;
        };
        self.header(ctx, frame, language);
        let language = self.language.unwrap_or(language);
        self.footer(ctx, language);
        let peak = load_f32(&self.stats.peak_bits);
        self.stats
            .peak_bits
            .store((peak * 0.90).to_bits(), Ordering::Relaxed);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.set_max_width(ui.available_width().min(900.0));
                            ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
                                if self.show_help {
                                    self.help(ui, language);
                                    ui.add_space(6.0);
                                }
                                self.devices(ui, language);
                                ui.add_space(6.0);
                                self.profiles(ui, language);
                                ui.add_space(6.0);
                                self.volume(ui, language, peak);
                                ui.add_space(6.0);
                                self.advanced(ui, language);
                                ui.add_space(12.0);
                            });
                        });
                    });
            });
        self.sync_controls();
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(language) = self.language {
            language.save(storage);
        }
    }
}
