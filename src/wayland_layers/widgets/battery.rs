use cosmic_text::{Color, FontSystem, SwashCache};
use smithay_client_toolkit::{
    compositor::CompositorHandler, shm::Shm, subcompositor::SubcompositorState,
};
use wayland_client::{QueueHandle, protocol::wl_surface};

use crate::wayland_layers::text_layer::TextLayer;
use crate::wayland_layers::traits::Layer;

pub struct Battery {
    layer: TextLayer,
}

impl Battery {
    pub fn new<D>(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        font_size: f32,
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
        Self {
            layer: TextLayer::new(x, y, width, height, font_size, parent, subcomp, shm, qh),
        }
    }

    pub fn tick(&mut self, font_system: &mut FontSystem, swash_cache: &mut SwashCache) {
        let percentage = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
            .unwrap_or_else(|_| "?".to_string())
            .replace('\n', "%");
        let actual_status = std::fs::read_to_string("/sys/class/power_supply/BAT0/status")
            .unwrap_or_else(|_| "?".to_string())
            .replace('\n', "");

        let status: char;
        if actual_status.eq_ignore_ascii_case("charging") {
            status = 'C';
        } else {
            status = 'D'
        }

        let mut text: String = String::with_capacity(5);
        text.insert(0, status);
        text.insert(1, '.');
        text.push_str(&percentage);

        self.layer.set_text(&text);
        self.layer
            .draw(font_system, swash_cache, Color::rgb(0xFF, 0xFF, 0xFF));
    }

    pub fn mark_dirty(&mut self) {
        self.layer.mark_dirty();
    }

    pub fn hide(&mut self) {
        self.layer.hide();
    }
}
