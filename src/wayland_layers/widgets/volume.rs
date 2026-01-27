use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;

use calloop::channel::{self, Sender};
use cosmic_text::{Color, FontSystem, SwashCache};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, SurfaceData},
    shm::Shm,
    subcompositor::{SubcompositorState, SubsurfaceData},
};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_subsurface, wl_surface},
};

use crate::wayland_layers::traits::Layer;
use crate::wayland_layers::{image_layer::ImageLayer, text_layer::TextLayer};

const ICON_VOLUME: &[u8] = include_bytes!("../../../assets/icons/volume-on.png");
const ICON_MUTE: &[u8] = include_bytes!("../../../assets/icons/volume-off.png");
const TEXT_COLOR: Color = Color::rgb(0xFF, 0xFF, 0xFF);

#[derive(Debug)]
pub struct VolumeState {
    pub percentage: u32,
    pub muted: bool,
}

pub struct Volume {
    text_layer: TextLayer,
    icon_layer: ImageLayer,
    muted: bool,
    icon_tint: Option<[u8; 3]>,
    _pactl: std::process::Child,
}

impl Volume {
    /// Returns the widget and a calloop Channel to register in the event loop.
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
    ) -> (Self, channel::Channel<VolumeState>)
    where
        D: CompositorHandler
            + Dispatch<wl_surface::WlSurface, SurfaceData>
            + Dispatch<wl_subsurface::WlSubsurface, SubsurfaceData>
            + 'static,
    {
        const ICON_SIZE: u32 = 24;
        const ICON_TEXT_GAP: i32 = 4;

        let icon_layer = ImageLayer::new(x, y, ICON_VOLUME, parent, subcomp, shm, qh);
        let text_layer = TextLayer::new(
            x + ICON_SIZE as i32 + ICON_TEXT_GAP,
            y,
            width,
            height,
            font_size,
            parent,
            subcomp,
            shm,
            qh,
        );

        let (sender, cal_channel) = channel::channel();
        let pactl = spawn_volume_watcher(sender);

        let mut volume = Self {
            text_layer,
            icon_layer,
            muted: false,
            icon_tint: Some([0xFF, 0xFF, 0xFF]),
            _pactl: pactl,
        };

        if let Some(state) = get_current_volume() {
            volume.apply_state(state, shm);
        }
        volume.mark_dirty();

        (volume, cal_channel)
    }

    pub fn apply_state(&mut self, state: VolumeState, shm: &Shm) {
        self.text_layer.set_text(&format!("{}%", state.percentage));

        if state.muted != self.muted {
            self.muted = state.muted;
            let icon = if self.muted { ICON_MUTE } else { ICON_VOLUME };
            self.icon_layer.set_image(icon, shm);
        }
    }

    pub fn hide_layers(&mut self) {
        self.text_layer.hide();
        self.icon_layer.hide();
    }

    pub fn draw(&mut self, font_system: &mut FontSystem, swash_cache: &mut SwashCache) {
        self.text_layer.draw(font_system, swash_cache, TEXT_COLOR);
        self.icon_layer.draw(self.icon_tint);
    }

    pub fn mark_dirty(&mut self) {
        self.text_layer.mark_dirty();
        self.icon_layer.mark_dirty();
    }

    pub fn pause(&mut self) {
        let pid = self._pactl.id() as i32;
        let pid = unsafe { rustix::process::Pid::from_raw_unchecked(pid) };
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::STOP);
    }

    pub fn resume(&mut self, shm: &Shm) {
        let pid = self._pactl.id() as i32;
        let pid = unsafe { rustix::process::Pid::from_raw_unchecked(pid) };
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::CONT);

        // Re-query since volume may have changed while suspended
        if let Some(state) = get_current_volume() {
            self.apply_state(state, shm);
        }
    }
}

fn spawn_volume_watcher(sender: Sender<VolumeState>) -> std::process::Child {
    let mut child = Command::new("pactl")
        .args(["subscribe"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn pactl subscribe");

    let stdout = child.stdout.take().unwrap();

    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if !line.contains("sink") && !line.contains("server") {
                continue;
            }
            if let Some(state) = get_current_volume() {
                if sender.send(state).is_err() {
                    break;
                }
            }
        }
    });

    child
}

fn get_current_volume() -> Option<VolumeState> {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;

    parse_wpctl_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_wpctl_output(output: &str) -> Option<VolumeState> {
    let mut parts = output.trim().split_whitespace();
    parts.next()?; // skip "Volume:"
    let volume: f32 = parts.next()?.parse().ok()?;
    let muted = output.contains("[MUTED]");
    Some(VolumeState {
        percentage: (volume * 100.0).round() as u32,
        muted,
    })
}
