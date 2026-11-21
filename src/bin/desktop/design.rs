use eframe::egui::{self, Color32, RichText, Stroke};

pub const INK: Color32 = Color32::from_rgb(24, 53, 57);
pub const MUTED: Color32 = Color32::from_rgb(76, 97, 98);
pub const PAPER: Color32 = Color32::from_rgb(246, 247, 243);
pub const LINE: Color32 = Color32::from_rgb(202, 213, 207);
pub const ACCENT: Color32 = Color32::from_rgb(255, 182, 104);

pub const ICON: &[u8] = include_bytes!("../../../packaging/art/tizen-ornn.png");
const LIBRARY: &[u8] = include_bytes!("../../../packaging/art/image-library.png");

pub struct Artwork {
    pub mark: egui::TextureHandle,
    pub library: egui::TextureHandle,
}

impl Artwork {
    pub fn load(ctx: &egui::Context) -> Self {
        let texture = |name, bytes| {
            let image = eframe::icon_data::from_png_bytes(bytes).expect("embedded artwork");
            let pixels = egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &image.rgba,
            );
            ctx.load_texture(name, pixels, egui::TextureOptions::LINEAR)
        };
        Self {
            mark: texture("ornn-mark", ICON),
            library: texture("image-library", LIBRARY),
        }
    }
}

pub fn wordmark() -> RichText {
    RichText::new("Tizen Ornn")
        .family(egui::FontFamily::Name("ornn-display".into()))
        .size(32.0)
        .color(Color32::WHITE)
}

pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let display = "Barlow Semi Condensed Bold";
    fonts.font_data.insert(
        display.into(),
        egui::FontData::from_static(include_bytes!(
            "../../../packaging/art/fonts/BarlowSemiCondensed-Bold.ttf"
        ))
        .into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("ornn-display".into()),
        vec![display.into()],
    );
    ctx.set_fonts(fonts);
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = PAPER;
    visuals.window_fill = PAPER;
    visuals.extreme_bg_color = Color32::WHITE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.override_text_color = Some(INK);
    visuals.selection.bg_fill = Color32::from_rgb(175, 220, 209);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
    });
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(10)
        .inner_margin(16)
}

pub fn action(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(INK).strong())
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(6)
}

pub fn secondary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(INK))
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(6)
}
