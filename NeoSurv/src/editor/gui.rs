#[cfg(feature = "editor")]
pub(crate) fn draw(ctx: &egui::Context) {
    egui::Window::new("Tokenburner Editor (WIP)").show(ctx, |ui| {
        ui.label("Editor ist vorbereitet, aber noch nicht im Runtime-Loop aktiviert.");
    });
}
