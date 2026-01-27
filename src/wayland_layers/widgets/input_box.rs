use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, SurfaceData},
    shm::{Shm, slot::SlotPool},
    subcompositor::{SubcompositorState, SubsurfaceData},
};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_shm, wl_subsurface, wl_surface},
};

use crate::drawing_util::rectangle::Rectangle;
use crate::theme;

const FONT_SIZE: f32 = 20.0;
const LINE_HEIGHT: f32 = FONT_SIZE * 1.2;
const MAX_INPUT_LEN: usize = 256;
const CARET_WIDTH: u32 = 10;
const CORNER_RADIUS: f32 = 6.0;
const TEXT_PADDING_X: i32 = 12;

pub struct InputBox {
    width: u32,
    height: u32,
    text: String,
    focused: bool,
    dirty: bool,
    surface: wl_surface::WlSurface,
    pool: SlotPool,
}

impl InputBox {
    pub fn new<D>(
        bar_width: u32,
        bar_height: u32,
        height: u32,
        parent: &wl_surface::WlSurface,
        subcomp: &SubcompositorState,
        shm: &Shm,
        qh: &QueueHandle<D>,
    ) -> Self
    where
        D: CompositorHandler
            + Dispatch<wl_surface::WlSurface, SurfaceData>
            + Dispatch<wl_subsurface::WlSubsurface, SubsurfaceData>
            + 'static,
    {
        let margin = 8u32;
        let width = bar_width - margin * 2;
        let x = margin as i32;
        let y = (bar_height - height - 8) as i32;

        let (subsurface, surface) = subcomp.create_subsurface(parent.clone(), qh);
        subsurface.set_position(x, y);
        subsurface.set_desync();

        let pool = SlotPool::new(width as usize * height as usize * 4, shm)
            .expect("Failed to create input box pool");

        Self {
            width,
            height,
            text: String::new(),
            focused: false,
            dirty: true,
            surface,
            pool,
        }
    }

    pub fn hide(&mut self) {
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.dirty = true;
    }

    /// Handle a printable character input.
    pub fn push_char(&mut self, c: char) {
        if !self.focused {
            return;
        }
        // Only accept printable non-control characters
        if c.is_control() {
            return;
        }
        if self.text.len() < MAX_INPUT_LEN {
            self.text.push(c);
            self.dirty = true;
        }
    }

    /// Handle backspace.
    pub fn backspace(&mut self) {
        if !self.focused {
            return;
        }
        // Pop a full char (handles multi-byte UTF-8 correctly)
        if self.text.pop().is_some() {
            self.dirty = true;
        }
    }

    /// Handle enter — fires the callback and clears input.
    pub fn enter(&mut self) -> Option<String> {
        if !self.focused || self.text.is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.text);
        self.dirty = true;
        Some(text)
    }

    /// Handle escape — clears input.
    pub fn escape(&mut self) {
        if !self.focused {
            return;
        }
        if !self.text.is_empty() {
            self.text.clear();
            self.dirty = true;
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            self.dirty = true;
        }
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn draw(&mut self, font_system: &mut FontSystem, swash_cache: &mut SwashCache) {
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
            .expect("Failed to create input box buffer");

        canvas.fill(0);

        // Background
        Rectangle::new(0, 0, width, height)
            .color(theme::BG_INPUT)
            .rounded(CORNER_RADIUS)
            .outline(1.0, theme::BORDER_SECONDARY, 255)
            .draw(canvas, width, height);

        // Shape text and measure caret position
        let metrics = Metrics::new(FONT_SIZE, LINE_HEIGHT);
        let mut text_buffer = Buffer::new(font_system, metrics);
        let mut text_buffer = text_buffer.borrow_with(font_system);

        text_buffer.set_size(Some(width as f32 - TEXT_PADDING_X as f32 * 2.0), None);

        // Show placeholder when empty and unfocused
        let display_text: &str = &self.text;

        text_buffer.set_text(display_text, &Attrs::new(), Shaping::Advanced, None);
        text_buffer.shape_until_scroll(true);

        // Measure text advance width for caret placement
        let text_advance = text_buffer
            .layout_runs()
            .next()
            .map(|run| run.line_w)
            .unwrap_or(0.0);

        let text_color = Color::rgb(
            theme::TEXT_PRIMARY[0],
            theme::TEXT_PRIMARY[1],
            theme::TEXT_PRIMARY[2],
        );

        // Vertical center
        let text_y_offset = ((height as f32 - LINE_HEIGHT) / 2.0) as i32;

        text_buffer.draw(swash_cache, text_color, |x, y, w, h, color| {
            let a = color.a();
            if a == 0 {
                return;
            }

            for py in y..y + h as i32 {
                for px in x..x + w as i32 {
                    let canvas_x = px + TEXT_PADDING_X;
                    let canvas_y = py + text_y_offset;

                    if canvas_x < 0
                        || canvas_x >= width as i32
                        || canvas_y < 0
                        || canvas_y >= height as i32
                    {
                        continue;
                    }

                    let row = stride as usize * canvas_y as usize;
                    let col = canvas_x as usize * 4;

                    let premul = |c: u8| (c as u32 * a as u32 / 255) as u8;

                    canvas[row + col] = premul(color.b());
                    canvas[row + col + 1] = premul(color.g());
                    canvas[row + col + 2] = premul(color.r());
                    canvas[row + col + 3] = a;
                }
            }
        });

        // Draw caret when focused
        if self.focused {
            let caret_x = TEXT_PADDING_X + text_advance as i32;
            let caret_height = (FONT_SIZE * 1.1) as u32;
            let caret_y = ((height - caret_height) / 2) as i32;

            // Clamp caret to stay within bounds
            if caret_x + CARET_WIDTH as i32 <= width as i32 {
                Rectangle::new(caret_x, caret_y, CARET_WIDTH, caret_height)
                    .color(theme::ACCENT)
                    .draw(canvas, width, height);
            }
        }

        self.surface
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(&self.surface)
            .expect("Failed to attach input box buffer");
        self.surface.commit();

        self.dirty = false;
    }
}
