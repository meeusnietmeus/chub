use std::sync::Arc;

use cosmic_text::{FontSystem, SwashCache};
use smithay_client_toolkit::{
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        WaylandSurface,
        wlr_layer::{KeyboardInteractivity, LayerSurface},
    },
    shm::{Shm, slot::SlotPool},
    subcompositor::SubcompositorState,
};
use wayland_client::{
    QueueHandle,
    protocol::{wl_compositor::WlCompositor, wl_keyboard, wl_pointer, wl_shm},
};

use crate::{
    command_dispatcher::CommandDispatcher,
    drawing_util, theme,
    wayland_layers::widgets::{
        battery::Battery, clock::Clock, input_box::InputBox, volume::Volume,
    },
};

pub struct BaseLayer {
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub shm: Shm,

    pub subcomp: Arc<SubcompositorState>,

    pub exit: bool,
    pub first_configure: bool,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub layer: LayerSurface,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub keyboard_focus: bool,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub qh: QueueHandle<BaseLayer>,

    pub font_system: FontSystem,
    pub text_swash_cache: SwashCache,

    pub visible: bool,

    // widget attribute?
    pub clock: Option<Clock>,
    pub battery: Option<Battery>,
    pub volume: Option<Volume>,
    pub input_box: Option<InputBox>,
    pub loop_handle: calloop::LoopHandle<'static, BaseLayer>,

    pub command_dispatcher: CommandDispatcher,
    pub wl_compositor: WlCompositor,
}

impl BaseLayer {
    pub fn toggle_visibility(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;

        let empty_region = self.wl_compositor.create_region(&self.qh, ());
        self.layer.set_input_region(Some(&empty_region));
        self.layer
            .set_keyboard_interactivity(KeyboardInteractivity::None);

        self.draw(true);

        if let Some(clock) = &mut self.clock {
            clock.hide();
        }
        if let Some(battery) = &mut self.battery {
            battery.hide();
        }
        if let Some(vol) = &mut self.volume {
            vol.hide_layers();
            vol.pause();
        }
        if let Some(input) = &mut self.input_box {
            input.hide();
        }
    }

    pub fn show(&mut self) {
        self.visible = true;

        self.layer.set_input_region(None);
        self.layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        // TODO: upon click outside of taskbar, close taskbar

        self.draw(false);

        self.clock.as_mut().unwrap().mark_dirty();
        self.clock
            .as_mut()
            .unwrap()
            .tick(&mut self.font_system, &mut self.text_swash_cache);

        self.battery.as_mut().unwrap().mark_dirty();
        self.battery
            .as_mut()
            .unwrap()
            .tick(&mut self.font_system, &mut self.text_swash_cache);

        let (vol, shm, fs, sc) = (
            &mut self.volume,
            &self.shm,
            &mut self.font_system,
            &mut self.text_swash_cache,
        );
        if let Some(vol) = vol {
            vol.resume(shm);
            vol.draw(fs, sc);
        }

        if let Some(input) = &mut self.input_box {
            input.mark_dirty();
            input.set_focused(true);
            input.draw(&mut self.font_system, &mut self.text_swash_cache);
        }
    }

    fn draw(&mut self, hide: bool) {
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
            .expect("Failed to create base layer buffer");

        if hide {
            canvas.fill(0);
        } else {
            drawing_util::rectangle::Rectangle::new(0, 0, width, height)
                .color(theme::BG_PRIMARY)
                .rounded(6.0)
                .outline(1.0, theme::BG_SECONDARY, 255)
                .draw(canvas, width, height);
        }

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("buffer attach");
        self.layer.commit();
    }
}
