pub trait Layer {
    fn mark_dirty(&mut self);
    fn hide(&mut self);
}
