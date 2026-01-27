use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use calloop::{
    EventLoop,
    generic::Generic,
    Interest, Mode, PostAction,
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use clap::Parser;
use cosmic_text::{FontSystem, SwashCache};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    },
    shm::{Shm, slot::SlotPool},
    subcompositor::SubcompositorState,
};
use wayland_client::{Connection, globals::registry_queue_init};

use wayland_layers::base_layer::BaseLayer;

use crate::command_dispatcher::CommandDispatcher;

mod command_dispatcher;
mod drawing_util;
mod instance;
mod ipc;
mod theme;
mod wayland_handlers;
mod wayland_layers;
mod config;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"), version, about = "A Wayland taskbar")]
struct Args {
    #[arg(long, help = "Run as daemon", conflicts_with = "toggle")]
    daemon: bool,

    #[arg(long, help = "Toggle visibility of running instance", conflicts_with = "daemon")]
    toggle: bool,

    #[arg(short, long, value_name = "FILE", help = "Path to config file")]
    config: Option<std::path::PathBuf>,
}

fn main() {
    let args = Args::parse();

    if args.toggle {
        ipc::send_toggle();
        return;
    }

    if !args.daemon {
        eprintln!(
            "usage: {} --daemon [--config <file>]\n       {} --toggle",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_NAME"),
        );
        std::process::exit(1);
    }

    // Daemon mode — enforce single instance
    let _instance_lock = match instance::acquire_lock() {
        Some(lock) => lock,
        None => {
            eprintln!("{} daemon is already running", env!("CARGO_PKG_NAME"));
            std::process::exit(1);
        }
    };

    let width = 700u32;
    let height = 110u32;

    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let subcompositor_state =
        SubcompositorState::bind(compositor.wl_compositor().clone(), &globals, &qh)
            .expect("wl_subcompositor not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm not available");

    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("toolbar"), None);
    layer.set_anchor(Anchor::BOTTOM);
    layer.set_margin(0, 0, 5, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(width, height);
    layer.commit();

    let pool = SlotPool::new(width as usize * height as usize * 4, &shm)
        .expect("Failed to create pool");

    let mut event_loop: EventLoop<'static, BaseLayer> =
        EventLoop::try_new().expect("Failed to create event loop");
    let loop_handle = event_loop.handle();

    let mut base_layer = BaseLayer {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        subcomp: Arc::new(subcompositor_state),
        exit: false,
        first_configure: true,
        visible: false,
        pool,
        width,
        height,
        layer,
        keyboard_focus: false,
        keyboard: None,
        pointer: None,
        font_system: FontSystem::new(),
        text_swash_cache: SwashCache::new(),
        clock: None,
        battery: None,
        volume: None,
        input_box: None,
        command_dispatcher: CommandDispatcher::new(),
        loop_handle: loop_handle.clone(),
        qh: qh.clone(),
        wl_compositor: compositor.wl_compositor().clone(),
    };

    if let Some(path) = args.config {
        base_layer.command_dispatcher.init_from_config(&path).expect("lol");
    }

    // Wayland events
    WaylandSource::new(conn, event_queue)
        .insert(loop_handle.clone())
        .expect("Failed to insert Wayland source");

    // Clock — 1 second, no-op when hidden
    loop_handle
        .insert_source(
            Timer::from_duration(Duration::from_secs(1)),
            |_, _, state: &mut BaseLayer| {
                if state.visible {
                    if let Some(clock) = &mut state.clock {
                        clock.tick(&mut state.font_system, &mut state.text_swash_cache);
                    }
                }
                TimeoutAction::ToDuration(Duration::from_secs(1))
            },
        )
        .expect("Failed to insert clock timer");

    // Battery — 30 seconds, no-op when hidden
    loop_handle
        .insert_source(
            Timer::from_duration(Duration::from_secs(30)),
            |_, _, state: &mut BaseLayer| {
                if state.visible {
                    if let Some(battery) = &mut state.battery {
                        battery.tick(&mut state.font_system, &mut state.text_swash_cache);
                    }
                }
                TimeoutAction::ToDuration(Duration::from_secs(30))
            },
        )
        .expect("Failed to insert battery timer");

    // IPC socket — listens for toggle messages
    let ipc_listener = ipc::bind_socket();
    loop_handle
        .insert_source(
            Generic::new(ipc_listener, Interest::READ, Mode::Level),
            |_, listener, state: &mut BaseLayer| {
                loop {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut buf = [0u8; 1];
                            if stream.read_exact(&mut buf).is_ok() {
                                match buf[0] {
                                    b't' => state.toggle_visibility(),
                                    _ => {}
                                }
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => eprintln!("IPC accept error: {}", e),
                    }
                }
                Ok(PostAction::Continue)
            },
        )
        .expect("Failed to insert IPC source");

    // TODO: days since update
    // TODO: network info
    // TODO: performance?
    // TODO: screen brightness
    // TODO: todo widget

    let loop_signal = event_loop.get_signal();
    event_loop
        .run(None, &mut base_layer, |state| {
            if state.exit {
                loop_signal.stop();
            }
        })
        .expect("Event loop error");
}
