use image::RgbaImage;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, SurfaceData},
    shm::{Shm, slot::SlotPool},
    subcompositor::{SubcompositorState, SubsurfaceData},
};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_shm, wl_subsurface, wl_surface},
};

use crate::wayland_layers::traits::Layer;

pub struct ImageLayer {
    width: u32,
    height: u32,
    surface: wl_surface::WlSurface,
    pool: SlotPool,
    dirty: bool,
    pixels: RgbaImage,
}

impl ImageLayer {
    pub fn new<D>(
        x: i32,
        y: i32,
        bytes: &[u8],
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
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode PNG")
            .into_rgba8();

        let width = img.width();
        let height = img.height();

        let (subsurface, surface) = subcomp.create_subsurface(parent.clone(), qh);
        subsurface.set_position(x, y);
        subsurface.set_desync();

        let pool = SlotPool::new(width as usize * height as usize * 4, shm)
            .expect("Failed to create image layer pool");

        Self {
            width,
            height,
            surface,
            pool,
            dirty: true,
            pixels: img,
        }
    }


    pub fn set_image(&mut self, bytes: &[u8], shm: &Shm) {
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode PNG")
            .into_rgba8();

        let new_size = img.width() as usize * img.height() as usize * 4;
        let current_size = self.width as usize * self.height as usize * 4;

        if new_size > current_size {
            self.pool =
                SlotPool::new(new_size, shm).expect("Failed to reallocate image layer pool");
        }

        self.width = img.width();
        self.height = img.height();
        self.pixels = img;
        self.dirty = true;
    }


    pub fn draw(&mut self, tint: Option<[u8; 3]>) {
        println!("image draw called, dirty = {}", self.dirty);

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
            .expect("Failed to create image layer buffer");

        canvas.fill(0);

        for (x, y, pixel) in self.pixels.enumerate_pixels() {
            let [r, g, b, a] = pixel.0;

            if a == 0 {
                continue;
            }

            // Apply tint by replacing RGB with tint color, preserving alpha
            let [out_r, out_g, out_b] = match tint {
                Some([tr, tg, tb]) => [tr, tg, tb],
                None => [r, g, b],
            };

            // Premultiply alpha
            let premul = |c: u8| (c as u32 * a as u32 / 255) as u8;

            let row = stride as usize * y as usize;
            let col = x as usize * 4;

            canvas[row + col] = premul(out_b);
            canvas[row + col + 1] = premul(out_g);
            canvas[row + col + 2] = premul(out_r);
            canvas[row + col + 3] = a;
        }

        println!("drew image");

        self.surface
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(&self.surface)
            .expect("Failed to attach image buffer");
        self.surface.commit();

        self.dirty = false;
    }
}

impl Layer for ImageLayer {
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    fn hide(&mut self) {
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.dirty = true;
    }
}
