#[derive(Default)]
pub struct EditorState {
    pub show_stats: bool,
    pub selected_entity: Option<u64>,
}

impl EditorState {
    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Inspector").show(ctx, |ui| {
            ui.label("MaxEngine Editor (WIP)");
            ui.checkbox(&mut self.show_stats, "Show stats");
            ui.label(format!("Selected entity: {:?}", self.selected_entity));
        });
    }
}
