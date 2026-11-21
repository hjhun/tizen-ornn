use eframe::egui::{self, Color32, RichText, Stroke};

pub const INK: Color32 = Color32::from_rgb(29, 32, 37);
pub const MUTED: Color32 = Color32::from_rgb(88, 95, 105);
pub const PAPER: Color32 = Color32::from_rgb(246, 247, 249);
const LINE: Color32 = Color32::from_rgb(215, 219, 225);
const ACCENT: Color32 = Color32::from_rgb(30, 83, 179);
pub const ICON: &[u8] = include_bytes!("../../../packaging/art/tizen-ornn.png");

pub fn configure(ctx: &egui::Context) {
    #[cfg(windows)]
    let font = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("Fonts/segoeui.ttf"))
        .and_then(|path| std::fs::read(path).ok());
    #[cfg(not(windows))]
    let font = [
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    .iter()
    .find_map(|path| std::fs::read(path).ok());
    if let Some(data) = font {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("system".into(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "system".into());
        ctx.set_fonts(fonts);
    }
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

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(12)
        .inner_margin(18)
}

pub fn action(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(8)
}

pub fn secondary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(INK))
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(8)
}
