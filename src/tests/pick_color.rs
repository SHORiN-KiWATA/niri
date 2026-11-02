use wayland_client::protocol::wl_pointer;

use super::*;

const BTN_LEFT: u32 = 0x110;

// The picker grab runs with the pointer locked, and rendering with the magnifier on locks the
// pointer too: capturing from inside the grab deadlocked niri.
#[test]
fn pick_color_with_magnifier() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.niri_state().backend.headless().add_renderer().unwrap();
    let id = f.add_client();

    f.niri().toggle_magnifier();
    f.niri_complete_animations();
    assert!(f.niri().magnifier_active);

    let (tx, rx) = async_channel::bounded(1);
    f.niri_state().handle_pick_color(tx, false);
    assert!(f.niri().pick_color_ui.is_open());

    let pointer = f.client(id).state.virtual_pointer();
    pointer.motion(0, 10., 10.);
    pointer.frame();
    f.roundtrip(id);
    pointer.axis_discrete(0, wl_pointer::Axis::VerticalScroll, 15., 1);
    pointer.frame();
    f.roundtrip(id);

    pointer.button(0, BTN_LEFT, wl_pointer::ButtonState::Pressed);
    pointer.frame();
    f.roundtrip(id);
    assert!(rx.try_recv().unwrap().is_some());
    assert!(!f.niri().pick_color_ui.is_open());
    // The grab waits for the release, so that it doesn't reach clients.
    assert!(f.niri().seat.get_pointer().unwrap().is_grabbed());

    pointer.button(0, BTN_LEFT, wl_pointer::ButtonState::Released);
    pointer.frame();
    f.roundtrip(id);
    assert!(!f.niri().seat.get_pointer().unwrap().is_grabbed());
}
