use std::num::NonZeroU32;

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, delegate_subcompositor,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_surface},
};

use crate::wayland_layers::{
    base_layer::BaseLayer,
    widgets::{battery::Battery, clock::Clock, input_box::InputBox, volume::Volume},
};

use calloop::channel::Event as ChannelEvent;

impl CompositorHandler for BaseLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for BaseLayer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for BaseLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.width = NonZeroU32::new(configure.new_size.0).map_or(self.width, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(self.height, NonZeroU32::get);

        if self.first_configure {
            self.first_configure = false;

            let parent = self.layer.wl_surface();

            self.clock = Some(Clock::new(
                6,
                4,
                180,
                self.height,
                parent,
                &self.subcomp,
                &self.shm,
                qh,
            ));
            self.battery = Some(Battery::new(
                140,
                11,
                180,
                self.height,
                22.0,
                parent,
                &self.subcomp,
                &self.shm,
                qh,
            ));

            let (volume, vol_channel) = Volume::new(
                240,
                11,
                80,
                self.height,
                22.0,
                parent,
                &self.subcomp,
                &self.shm,
                qh,
            );
            self.volume = Some(volume);

            self.loop_handle
                .insert_source(vol_channel, |event, _, state: &mut BaseLayer| {
                    if let ChannelEvent::Msg(vol_state) = event {
                        if !state.visible {
                            return;
                        }
                        let (vol, shm, font_system, swash_cache) = (
                            &mut state.volume,
                            &state.shm,
                            &mut state.font_system,
                            &mut state.text_swash_cache,
                        );
                        if let Some(vol) = vol {
                            vol.apply_state(vol_state, shm);
                            vol.draw(font_system, swash_cache);
                        }
                    }
                })
                .expect("Failed to insert volume channel");

            self.input_box = Some(InputBox::new(
                self.width,
                self.height,
                44,
                parent,
                &self.subcomp,
                &self.shm,
                qh,
            ));

            self.hide();
        }
    }
}

impl SeatHandler for BaseLayer {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
        }

        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            self.keyboard.take().unwrap().release();
        }
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl Dispatch<wl_region::WlRegion, ()> for BaseLayer {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl KeyboardHandler for BaseLayer {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        keysyms: &[Keysym],
    ) {
        if self.layer.wl_surface() == surface {
            println!("Keyboard focus on window with pressed syms: {keysyms:?}");
            self.keyboard_focus = true;
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self.layer.wl_surface() == surface {
            println!("Release keyboard focus on window");
            self.keyboard_focus = false;
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::Escape {
            if let Some(input) = &mut self.input_box {
                if input.is_focused() {
                    input.escape();
                } else {
                    self.exit = true;
                }
            }
        }

        if event.keysym == Keysym::BackSpace {
            if let Some(input) = &mut self.input_box {
                input.backspace();
            }
        }

        if event.keysym == Keysym::Return {
            if let Some(input) = &mut self.input_box {
                if let Some(cmd) = input.enter() {
                    self.command_dispatcher.dispatch(&cmd);
                }
            }
        }

        // Push printable characters — filter out keysyms with modifiers
        // key_char() returns None for non-printable keys and key combos
        if let Some(c) = event.utf8.and_then(|s| {
            let mut chars = s.chars();
            let c = chars.next()?;

            if chars.next().is_some() {
                return None;
            }
            Some(c)
        }) {
            if let Some(input) = &mut self.input_box {
                input.push_char(c);
            }
        }

        let (input, fs, sc) = (
            &mut self.input_box,
            &mut self.font_system,
            &mut self.text_swash_cache,
        );
        if let Some(input) = input {
            input.draw(fs, sc);
        }
    }
    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _raw_modifiers: RawModifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for BaseLayer {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    println!("Pointer entered @{:?}", event.position);
                }
                PointerEventKind::Leave { .. } => {
                    println!("Pointer left");
                }
                PointerEventKind::Motion { .. } => {}
                PointerEventKind::Press { button, .. } => {
                    println!("Press {:x} @ {:?}", button, event.position);
                }
                PointerEventKind::Release { button, .. } => {
                    println!("Release {:x} @ {:?}", button, event.position);
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    println!("Scroll H:{horizontal:?}, V:{vertical:?}");
                }
            }
        }
    }
}

impl ShmHandler for BaseLayer {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for BaseLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(BaseLayer);
delegate_subcompositor!(BaseLayer);
delegate_output!(BaseLayer);
delegate_shm!(BaseLayer);
delegate_seat!(BaseLayer);
delegate_keyboard!(BaseLayer);
delegate_pointer!(BaseLayer);
delegate_layer!(BaseLayer);
delegate_registry!(BaseLayer);
