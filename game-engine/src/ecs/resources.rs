#[derive(Debug, Clone, Copy)]
pub(crate) struct Time {
    pub(crate) delta_seconds: f32,
    pub(crate) frame_index: u64,
}
