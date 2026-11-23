use eframe::egui::{self, Color32, RichText, Stroke};

pub const INK: Color32 = Color32::from_rgb(29, 32, 37);
pub const MUTED: Color32 = Color32::from_rgb(88, 95, 105);
pub const PAPER: Color32 = Color32::from_rgb(246, 247, 249);
const LINE: Color32 = Color32::from_rgb(215, 219, 225);
pub const ACCENT: Color32 = Color32::from_rgb(32, 82, 168);
pub const ICON: &[u8] = include_bytes!("../../../packaging/art/tizen-ornn.png");

pub fn configure(ctx: &egui::Context) {
    #[cfg(windows)]
    let faces = {
        let root =
            std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("Fonts");
        [root.join("segoeui.ttf"), root.join("seguisb.ttf")]
    };
    #[cfg(not(windows))]
    let faces = {
        let root = std::path::Path::new("/usr/share/fonts/truetype");
        if root.join("lato/Lato-Regular.ttf").is_file() {
            [
                root.join("lato/Lato-Regular.ttf"),
                root.join("lato/Lato-Semibold.ttf"),
            ]
        } else if root.join("ubuntu/Ubuntu-R.ttf").is_file() {
            [
                root.join("ubuntu/Ubuntu-R.ttf"),
                root.join("ubuntu/Ubuntu-M.ttf"),
            ]
        } else {
            [
                root.join("dejavu/DejaVuSans.ttf"),
                root.join("dejavu/DejaVuSans-Bold.ttf"),
            ]
        }
    };
    let mut fonts = egui::FontDefinitions::default();
    let family = egui::FontFamily::Proportional;
    if let Ok(data) = std::fs::read(&faces[0]) {
        fonts
            .font_data
            .insert("system".into(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(family.clone())
            .or_default()
            .insert(0, "system".into());
    }
    #[cfg(windows)]
    let korean = faces[0].with_file_name("malgun.ttf");
    #[cfg(not(windows))]
    let korean = std::path::PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc");
    if let Ok(data) = std::fs::read(korean) {
        let face = egui::FontData::from_owned(data);
        // The second face in Noto Sans CJK is the Korean variant.
        #[cfg(not(windows))]
        let face = egui::FontData { index: 1, ..face };
        fonts.font_data.insert("korean".into(), face.into());
        for list in fonts.families.values_mut() {
            list.push("korean".into());
        }
    }
    let mut headings = fonts.families[&family].clone();
    if let Ok(data) = std::fs::read(&faces[1]) {
        fonts
            .font_data
            .insert("semibold".into(), egui::FontData::from_owned(data).into());
        headings.insert(0, "semibold".into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("heading".into()), headings);
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = PAPER;
        style.visuals.window_fill = Color32::WHITE;
        style.visuals.extreme_bg_color = Color32::WHITE;
        style.visuals.selection.bg_fill = Color32::from_rgb(228, 237, 252);
        style.visuals.override_text_color = Some(INK);
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.spacing.button_padding = egui::vec2(14.0, 7.0);
        style.spacing.interact_size.y = 35.0;
        for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
            style
                .text_styles
                .insert(text, egui::FontId::proportional(15.0));
        }
    });
}

pub fn heading(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(egui::FontId::new(
        size,
        egui::FontFamily::Name("heading".into()),
    ))
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(10)
        .inner_margin(16)
}

pub fn action(text: &str) -> egui::Button<'_> {
    egui::Button::new(heading(text, 15.0).color(Color32::WHITE))
        .min_size(egui::vec2(0.0, 40.0))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(8)
}

pub fn secondary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).size(15.0).color(INK))
        .min_size(egui::vec2(0.0, 40.0))
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(8)
}
