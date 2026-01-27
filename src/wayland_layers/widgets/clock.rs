use chrono::{Local, SubsecRound};
use cosmic_text::{Color, FontSystem, SwashCache};
use smithay_client_toolkit::{
    compositor::CompositorHandler, shm::Shm, subcompositor::SubcompositorState,
};
use wayland_client::{QueueHandle, protocol::wl_surface};

use crate::wayland_layers::text_layer::TextLayer;
use crate::wayland_layers::traits::Layer;

const TIME_FONT_SIZE: f32 = 22.0;
const DATE_FONT_SIZE: f32 = 14.0;
const SUB_TEXT_COLOR: Color = Color::rgb(0xCC, 0xCC, 0xCC);
const PRIMARY_TEXT_COLOR: Color = Color::rgb(0xFF, 0xFF, 0xFF);

pub struct Clock {
    time_layer: TextLayer,
    date_layer: TextLayer,
}

impl Clock {
    pub fn new<D>(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        parent: &wl_surface::WlSurface,
        subcomp: &SubcompositorState,
        shm: &Shm,
        qh: &QueueHandle<D>,
    ) -> Self
    where
        D: CompositorHandler
            + wayland_client::Dispatch<
                wl_surface::WlSurface,
                smithay_client_toolkit::compositor::SurfaceData,
            > + wayland_client::Dispatch<
                wayland_client::protocol::wl_subsurface::WlSubsurface,
                smithay_client_toolkit::subcompositor::SubsurfaceData,
            > + 'static,
    {
        // Date sits at the top, time below it
        let date_height = (DATE_FONT_SIZE * 1.2).ceil() as u32;
        let time_height = height.saturating_sub(date_height);

        let date_layer = TextLayer::new(
            x,
            y,
            width,
            date_height,
            DATE_FONT_SIZE,
            parent,
            subcomp,
            shm,
            qh,
        );
        let time_layer = TextLayer::new(
            x,
            y + date_height as i32,
            width,
            time_height,
            TIME_FONT_SIZE,
            parent,
            subcomp,
            shm,
            qh,
        );

        Self {
            time_layer,
            date_layer,
        }
    }

    /// Call every second. Date layer only redraws when the date string changes.
    pub fn tick(&mut self, font_system: &mut FontSystem, swash_cache: &mut SwashCache) {
        let now = Local::now();

        // set_text is a no-op if the date hasn't changed, draw() checks dirty flag
        let date = now.format("%A %d").to_string();
        self.date_layer.set_text(&date);
        self.date_layer
            .draw(font_system, swash_cache, SUB_TEXT_COLOR);

        let time = now.time().trunc_subsecs(0).to_string();
        self.time_layer.set_text(&time);
        self.time_layer
            .draw(font_system, swash_cache, PRIMARY_TEXT_COLOR);
    }

    pub fn mark_dirty(&mut self) {
        self.time_layer.mark_dirty();
        self.date_layer.mark_dirty();
    }
    pub fn hide(&mut self) {
        self.time_layer.hide();
        self.date_layer.hide();
    }

}
