use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight};
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    shm::{Shm, slot::SlotPool},
    subcompositor::SubcompositorState,
};
use wayland_client::{
    QueueHandle,
    protocol::{wl_shm, wl_surface},
};

use crate::wayland_layers::traits::Layer;

pub struct TextLayer {
    width: u32,
    height: u32,
    font_size: f32,
    text: String,
    dirty: bool,
    surface: wl_surface::WlSurface,
    pool: SlotPool,
}

impl TextLayer {
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
        let (subsurface, surface) = subcomp.create_subsurface(parent.clone(), qh);

        subsurface.set_position(x, y);

        subsurface.set_desync();

        let pool = SlotPool::new(width as usize * height as usize * 4, shm)
            .expect("Failed to create text layer pool");

        Self {
            width,
            height,
            font_size,
            text: String::new(),
            dirty: true,
            surface,
            pool,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_string();
            self.dirty = true;
        }
    }

    pub fn draw(
        &mut self,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        color: Color,
    ) {
        if !self.dirty {
            return;
        }

        let width = self.width;
        let height = self.height;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("Failed to create text layer buffer");

        // Clear canvas to transparent
        canvas.fill(0);

        let line_height = self.font_size * 1.2;
        let metrics = Metrics::new(self.font_size, line_height);

        let mut text_buffer = Buffer::new(font_system, metrics);
        let mut text_buffer = text_buffer.borrow_with(font_system);

        text_buffer.set_size(Some(width as f32), Some(height as f32));
        text_buffer.set_text(
            &self.text,
            &Attrs::new()
                .family(cosmic_text::Family::Monospace)
                .weight(Weight::MEDIUM),
            Shaping::Advanced,
            None,
        );
        text_buffer.shape_until_scroll(true);

        text_buffer.draw(swash_cache, color, |x, y, w, h, color| {
            let a = color.a();
            if a == 0 {
                return;
            }

            for py in y..y + h as i32 {
                for px in x..x + w as i32 {
                    if px < 0 || px >= width as i32 || py < 0 || py >= height as i32 {
                        continue;
                    }

                    let row = stride as usize * py as usize;
                    let col = px as usize * 4;

                    // Premultiply alpha before writing into ARGB8888 buffer
                    let premul = |c: u8| (c as u32 * a as u32 / 255) as u8;

                    canvas[row + col] = premul(color.b());
                    canvas[row + col + 1] = premul(color.g());
                    canvas[row + col + 2] = premul(color.r());
                    canvas[row + col + 3] = a;
                }
            }
        });

        self.surface
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(&self.surface)
            .expect("Failed to attach text buffer");
        self.surface.commit();

        self.dirty = false;
    }
}

impl Layer for TextLayer {
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    fn hide(&mut self) {
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.dirty = true;
    }
}
