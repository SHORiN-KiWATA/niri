use std::sync::Arc;

use niri_ipc::PickedColor;
use smithay::backend::input::ButtonState;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, CursorImageStatus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
    MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
};
use smithay::input::SeatHandler;
use smithay::utils::{Logical, Point};
use smithay::wayland::selection::data_device::set_data_device_selection;

use crate::niri::State;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

pub struct PickColorGrab {
    start_data: PointerGrabStartData<State>,
    copy_to_clipboard: bool,
    /// Buttons pressed during the grab. Their presses and releases never reach clients.
    own_buttons: Vec<u32>,
    /// The pick is over; the grab only waits for `own_buttons` to be released, so that the
    /// releases don't reach clients and the pointer's pressed buttons stay in sync.
    done: bool,
}

impl PickColorGrab {
    pub fn new(start_data: PointerGrabStartData<State>, copy_to_clipboard: bool) -> Self {
        Self {
            start_data,
            copy_to_clipboard,
            own_buttons: Vec::new(),
            done: false,
        }
    }

    /// Whether a button pressed before the grab started is still held, e.g. the click on a bar
    /// button that launched the picker.
    fn initiating_button_held(&self, handle: &PointerInnerHandle<'_, State>) -> bool {
        handle
            .current_pressed()
            .iter()
            .any(|button| !self.own_buttons.contains(button))
    }

    /// The client holding an initiating button keeps the pointer until the button is released,
    /// like an implicit grab; everyone else loses it while picking.
    fn focus(
        &self,
        handle: &PointerInnerHandle<'_, State>,
    ) -> Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)> {
        if self.initiating_button_held(handle) {
            handle.current_focus()
        } else {
            None
        }
    }

    fn finish(&mut self, state: &mut State) {
        self.done = true;
        if let Some(tx) = state.niri.pick_color.take() {
            let _ = tx.send_blocking(None);
        }
        state.niri.pick_color_ui.close();
        state
            .niri
            .cursor_manager
            .set_cursor_image(CursorImageStatus::default_named());
        state.niri.queue_redraw_all();
    }

    fn pick_color_at_point(location: Point<f64, Logical>, data: &mut State) -> Option<PickedColor> {
        let (output, pos_within_output) = data.niri.output_under(location)?;
        data.niri.pick_color_ui.color_at(output, pos_within_output)
    }

    fn copy_to_clipboard(color: &PickedColor, data: &mut State) {
        let [r, g, b] = color.rgb.map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
        let text: Arc<[u8]> = Arc::from(format!("#{r:02x}{g:02x}{b:02x}").into_bytes());
        set_data_device_selection(
            &data.niri.display_handle,
            &data.niri.seat,
            vec![
                String::from("text/plain;charset=utf-8"),
                String::from("text/plain"),
                String::from("UTF8_STRING"),
                String::from("STRING"),
                String::from("TEXT"),
            ],
            text,
        );
    }
}

impl PointerGrab<State> for PickColorGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        let focus = self.focus(handle);
        handle.motion(data, focus, event);
        if !self.done {
            data.update_pick_color_magnifier(event.location);
            data.niri.queue_redraw_all();
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        let focus = self.focus(handle);
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &ButtonEvent,
    ) {
        match event.state {
            ButtonState::Pressed => {
                self.own_buttons.push(event.button);
                if !self.done {
                    match event.button {
                        BTN_LEFT => {
                            if let Some(tx) = data.niri.pick_color.take() {
                                let color =
                                    Self::pick_color_at_point(handle.current_location(), data);
                                if self.copy_to_clipboard {
                                    if let Some(color) = &color {
                                        Self::copy_to_clipboard(color, data);
                                    }
                                }
                                let _ = tx.send_blocking(color);
                            }
                            self.finish(data);
                        }
                        // Right click cancels.
                        BTN_RIGHT => self.finish(data),
                        _ => (),
                    }
                }
            }
            ButtonState::Released => {
                if let Some(idx) = self.own_buttons.iter().position(|b| *b == event.button) {
                    self.own_buttons.remove(idx);
                } else {
                    // The client saw this button's press, so it gets the release too.
                    handle.button(data, event);
                    if !self.initiating_button_held(handle) {
                        let location = handle.current_location();
                        let motion = MotionEvent {
                            location,
                            serial: event.serial,
                            time: event.time,
                        };
                        handle.motion(data, None, &motion);
                    }
                }
            }
        }

        if self.done && handle.current_pressed().is_empty() {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        details: AxisFrame,
    ) {
        // Scrolling zooms the magnifier, like hyprpicker.
        let steps = match details.v120 {
            Some((_, v120)) => f64::from(v120) / 120.,
            None => details.axis.1 / 15.,
        };
        if steps != 0. {
            data.niri.pick_color_ui.zoom(-steps);
            data.update_pick_color_magnifier(handle.current_location());
            data.niri.queue_redraw_all();
        }
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &PointerGrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        // Once done, a new picker may already be open.
        if !self.done {
            self.finish(data);
        }
    }
}
