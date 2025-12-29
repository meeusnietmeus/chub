use std::{time::Instant};

use cosmic_text::{FontSystem, SwashCache};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState,
        pointer::PointerEventKind,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm},
};
use wayland_client::{
    Connection, globals::registry_queue_init
};

use wayland_layers::simplelayer::SimpleLayer;

mod wayland_layers;

fn main() {
    let width = 700;
    let height = 80;

    // All Wayland apps start by connecting the compositor (server).
    let conn = Connection::connect_to_env().unwrap();

    // Enumerate the list of globals to get the protocols the server implements.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    // The compositor (not to be confused with the server which is commonly called the compositor) allows
    // configuring surfaces to be presented.
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    // This app uses the wlr layer shell, which may not be available with every compositor.
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    // Since we are not using the GPU in this example, we use wl_shm to allow software rendering to a buffer
    // we share with the compositor process.
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    // A layer surface is created from a surface.
    let surface = compositor.create_surface(&qh);

    // And then we create the layer shell.
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("toolbar"), None);
    // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
    // interactivity
    layer.set_anchor(Anchor::BOTTOM);
    layer.set_margin(0, 0, 5, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(width, height);

    // In order for the layer surface to be mapped, we need to perform an initial commit with no attached\
    // buffer. For more info, see WaylandSurface::commit
    //
    // The compositor will respond with an initial configure that we can then use to present to the layer
    // surface with the correct options.
    layer.commit();

    // We don't know how large the window will be yet, so lets assume the minimum size we suggested for the
    // initial memory allocation.
    let pool = SlotPool::new(100 * 100 * 4, &shm).expect("Failed to create pool");

    let mut simple_layer = SimpleLayer {
        // Seats and outputs may be hotplugged at runtime, therefore we need to setup a registry state to
        // listen for seats and outputs.
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,

        exit: false,
        first_configure: true,
        pool,
        width,
        height,
        layer,
        keyboard: None,
        keyboard_focus: false,
        pointer: None,

        font_system: FontSystem::new(),
        text_swash_cache: SwashCache::new(),
    };

    //TODO: check last part of EventQueue documentation -> epoll gebruike?
    let mut now = Instant::now();
    while !simple_layer.exit {
        if !simple_layer.first_configure {
            if now.elapsed().as_secs() >= 1 {
                now = Instant::now();
                println!("redraw to update systime");
                simple_layer.layer.wl_surface().frame(&qh, simple_layer.layer.wl_surface().clone());
                simple_layer.layer.commit();
            }
        }

        // Should also redraw on every keystroke (only the input)
        event_queue.blocking_dispatch(&mut simple_layer).unwrap();
    }
}

