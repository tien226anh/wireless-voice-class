use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

const LOGO_PNG: &[u8] = include_bytes!("../assets/icons/wireless-pa.png");
const TEXTURE_KEY: &str = "wireless-pa.logo";

pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(LOGO_PNG).expect("Bundled application icon must be valid PNG")
}

pub fn install(ctx: &egui::Context) {
    let icon = window_icon();
    let image =
        ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let texture = ctx.load_texture(
        TEXTURE_KEY,
        image,
        TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(TEXTURE_KEY), texture));
}

pub fn logo(ui: &mut egui::Ui, size: f32) {
    let texture = ui.ctx().data(|data| {
        data.get_temp::<TextureHandle>(egui::Id::new(TEXTURE_KEY))
            .expect("Branding must be installed before drawing the UI")
    });
    ui.add(egui::Image::new(&texture).fit_to_exact_size(egui::Vec2::splat(size)));
}
