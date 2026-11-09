//! Takeover: the user operating windows of agent apps.
//!
//! Agent apps see only the agent seat, so the user's own seat can't reach their windows. niri
//! hands the user's input on them to the app through the agent seat instead. Pressing a button or
//! key on such a window takes it over: agent input is refused until the user's keyboard focus
//! leaves the window, and the clipboards of the two seats are synced in the meantime.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::backend::input::{ButtonState, InputTime, KeyState};
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::input::Seat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource as _;
use smithay::utils::{IsAlive, Logical, Point, SERIAL_COUNTER};
use smithay::wayland::input_method::InputMethodSeat;
use smithay::wayland::selection::data_device::{
    clear_data_device_selection, current_data_device_selection_userdata,
    request_data_device_client_selection, set_data_device_selection,
};
use smithay::wayland::selection::primary_selection::{
    clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection,
    set_primary_selection,
};
use smithay::wayland::selection::{SelectionSource, SelectionTarget};

use super::is_agent_surface;
use crate::niri::State;

/// How long after the last agent action the user's hovering reaches agent apps again.
///
/// Until then, the agent pointer stays where the agent left it, so that the user passing over an
/// agent window doesn't move it in the middle of an action.
const HOVER_AFTER: Duration = Duration::from_secs(2);

const TARGETS: [SelectionTarget; 2] = [SelectionTarget::Clipboard, SelectionTarget::Primary];

#[derive(Default)]
pub struct Takeover {
    /// Toplevel that the user took over.
    window: Option<WlSurface>,
    /// Whether the user's pointer drives the agent pointer.
    pointer: bool,
    /// Keys that the user pressed through the agent keyboard.
    keys: Vec<Keycode>,
    /// When the agent last sent input.
    last_action: Option<Instant>,
    /// Selections that niri set on one seat to pass on the other seat's.
    mirrors: Vec<Mirror>,
    /// Mime types of the latest selection that a client set, per seat and target.
    selections: Vec<(Side, SelectionTarget, Vec<String>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    User,
    Agent,
}

impl Side {
    fn other(self) -> Self {
        match self {
            Side::User => Side::Agent,
            Side::Agent => Side::User,
        }
    }
}

struct Mirror {
    /// User data of the selection, to recognize it by.
    token: Arc<[u8]>,
    /// Seat that holds the mirror; it passes on the other seat's selection.
    on: Side,
    ty: SelectionTarget,
}

impl State {
    /// Refuses agent input while the user has taken over a window, and records the action.
    pub(super) fn agent_check_takeover(&mut self) -> Result<(), String> {
        if let Some(window) = self.agent_takeover_window() {
            let window = match self.niri.layout.find_window_and_output(&window) {
                Some((mapped, _)) => format!("window {}", mapped.id().get()),
                None => String::from("an agent window"),
            };
            return Err(format!(
                "the user took over {window} and is using it; agent input waits until they \
                 switch away from it"
            ));
        }

        let takeover = &mut self.agent().takeover;
        takeover.last_action = Some(Instant::now());
        takeover.pointer = false;
        Ok(())
    }

    /// The window that the user took over, if it's still around.
    fn agent_takeover_window(&mut self) -> Option<WlSurface> {
        let window = self.niri.agent.as_ref()?.takeover.window.clone()?;
        if window.alive() {
            Some(window)
        } else {
            self.agent_end_takeover();
            None
        }
    }

    /// Whether the user's pointer drives the agent pointer, so the cursor that the agent app sets
    /// is the user's.
    pub fn agent_user_drives_pointer(&self) -> bool {
        self.niri
            .agent
            .as_ref()
            .is_some_and(|agent| agent.takeover.pointer)
    }

    /// The agent window that takes the user's input at `surface`.
    fn agent_user_target(&self, surface: &WlSurface) -> Option<WlSurface> {
        if !is_agent_surface(surface) {
            return None;
        }
        let root = self.niri.find_root_shell_surface(surface);
        self.niri
            .layout
            .find_window_and_output(&root)
            .is_some()
            .then_some(root)
    }

    fn agent_start_takeover(&mut self, window: WlSurface) {
        if self.niri.agent.as_ref().unwrap().takeover.window.as_ref() == Some(&window) {
            return;
        }
        self.agent_end_takeover();
        debug!("the user took over an agent window");

        // The user's own cursor is right there.
        let agent = self.agent();
        if agent.cursor_window.as_ref() == Some(&window) {
            agent.cursor_window = None;
        }
        if let Some((mapped, output)) = self.niri.layout.find_window_and_output_mut(&window) {
            if mapped.agent_cursor.take().is_some() {
                match output.cloned() {
                    Some(output) => self.niri.queue_redraw(&output),
                    None => self.niri.queue_redraw_all(),
                }
            }
        }

        self.agent_give_keyboard(&window);
        self.agent().takeover.window = Some(window);

        // Let the window paste what the user copied.
        let user_seat = self.niri.seat.clone();
        for ty in TARGETS {
            let compositor_owned = match ty {
                SelectionTarget::Clipboard => {
                    current_data_device_selection_userdata(&user_seat).is_some()
                }
                SelectionTarget::Primary => {
                    current_primary_selection_userdata(&user_seat).is_some()
                }
            };
            // niri's own selections (screenshots) and mirrors of the agent's aren't passed on.
            if compositor_owned {
                continue;
            }
            let takeover = &self.agent().takeover;
            if let Some((_, _, mimes)) = takeover
                .selections
                .iter()
                .find(|(side, t, _)| *side == Side::User && *t == ty)
            {
                let mimes = mimes.clone();
                self.agent_mirror_selection(Side::Agent, ty, mimes);
            }
        }
    }

    /// Hands the window that the user took over back to the agent.
    fn agent_end_takeover(&mut self) {
        let Some(agent) = &mut self.niri.agent else {
            return;
        };
        if agent.takeover.window.take().is_none() {
            return;
        }
        debug!("the user handed an agent window back");

        let seat = agent.seat.clone();
        let keys = std::mem::take(&mut agent.takeover.keys);
        let keyboard = seat.get_keyboard().unwrap();
        for code in keys {
            keyboard.input(
                self,
                code,
                KeyState::Released,
                SERIAL_COUNTER.next_serial(),
                InputTime::now(),
                |_, _, _| FilterResult::<()>::Forward,
            );
        }

        // The agent app stops seeing the user's clipboard.
        let agent = self.agent();
        let mut cleared = Vec::new();
        agent.takeover.mirrors.retain(|mirror| {
            if mirror.on == Side::Agent {
                cleared.push((mirror.ty, mirror.token.clone()));
                false
            } else {
                true
            }
        });
        let dh = self.niri.display_handle.clone();
        for (ty, token) in cleared {
            match ty {
                SelectionTarget::Clipboard => {
                    let current = current_data_device_selection_userdata(&seat)
                        .is_some_and(|data| Arc::ptr_eq(&data, &token));
                    if current {
                        clear_data_device_selection(&dh, &seat);
                    }
                }
                SelectionTarget::Primary => {
                    let current = current_primary_selection_userdata(&seat)
                        .is_some_and(|data| Arc::ptr_eq(&data, &token));
                    if current {
                        clear_primary_selection(&dh, &seat);
                    }
                }
            }
        }
    }

    /// Gives the agent keyboard focus to the window, unless one of its popups holds it.
    fn agent_give_keyboard(&mut self, window: &WlSurface) {
        let seat = self.agent_seat();
        let keyboard = seat.get_keyboard().unwrap();
        if !self.agent_popup_grab_on(window) && keyboard.current_focus().as_ref() != Some(window) {
            keyboard.set_focus(self, Some(window.clone()), SERIAL_COUNTER.next_serial());
        }
    }

    /// Follows the user's keyboard focus: the takeover moves along to other windows of the same
    /// app, like its dialogs, and ends when the focus goes anywhere else.
    pub fn agent_on_user_focus(&mut self, focus: Option<&WlSurface>) {
        let Some(agent) = &self.niri.agent else {
            return;
        };
        let takeover = &agent.takeover;
        let window = takeover.window.clone();
        let idle = takeover
            .last_action
            .is_none_or(|at| at.elapsed() >= HOVER_AFTER);
        let target = focus.and_then(|surface| self.agent_user_target(surface));

        match (window, target) {
            (Some(window), Some(target)) if window == target => (),
            (Some(window), Some(target)) if window.client() == target.client() => {
                self.agent_give_keyboard(&target);
                self.agent().takeover.window = Some(target);
            }
            (Some(_), target) => {
                self.agent_end_takeover();
                if let Some(target) = target.filter(|_| idle) {
                    self.agent_give_keyboard(&target);
                }
            }
            // Focus the agent app right away, so that the first key the user types doesn't come
            // together with the focus, which GTK 3 drops.
            (None, Some(target)) if idle => self.agent_give_keyboard(&target),
            (None, _) => (),
        }
    }

    /// Whether the user operates the app of this agent surface, so that its new windows may take
    /// the focus.
    pub fn agent_user_operates(&self, surface: &WlSurface) -> bool {
        self.niri.agent.as_ref().is_some_and(|agent| {
            agent
                .takeover
                .window
                .as_ref()
                .is_some_and(|window| window.alive() && window.client() == surface.client())
        })
    }

    /// Passes the user's pointer motion on to the agent app under the pointer.
    pub fn agent_user_motion(
        &mut self,
        under: Option<(WlSurface, Point<f64, Logical>)>,
        location: Point<f64, Logical>,
        time: InputTime,
    ) {
        let Some(agent) = &self.niri.agent else {
            return;
        };
        let takeover = &agent.takeover;
        let target = under
            .as_ref()
            .and_then(|(surface, _)| self.agent_user_target(surface));

        let forward = match (&takeover.window, &target) {
            (Some(window), Some(target)) => window == target || takeover.pointer,
            // Hovering reaches agent apps when the agent isn't busy with its pointer.
            (None, Some(_)) => {
                takeover.pointer
                    || takeover
                        .last_action
                        .is_none_or(|at| at.elapsed() >= HOVER_AFTER)
            }
            // Leaving an agent window, or dragging out of it.
            (_, None) => takeover.pointer,
        };
        if !forward {
            return;
        }

        let focus = target.is_some().then_some(under).flatten();
        let seat = self.agent_seat();
        let pointer = seat.get_pointer().unwrap();
        pointer.motion(
            self,
            focus,
            &MotionEvent {
                location,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
        pointer.frame(self);

        // An implicit grab keeps the focus while the user drags out of the window.
        let drives = pointer.current_focus().is_some();
        self.agent().takeover.pointer = drives;
    }

    /// Passes a button of the user's pointer on to the agent app under it.
    pub fn agent_user_button(&mut self, button: u32, state: ButtonState, time: InputTime) {
        if self.niri.agent.is_none() {
            return;
        }

        if state == ButtonState::Pressed {
            let under = self.niri.pointer_contents.surface.clone();
            let target = under
                .as_ref()
                .and_then(|(surface, _)| self.agent_user_target(surface));
            match target {
                Some(target) => {
                    self.agent_start_takeover(target);
                    if !self.agent().takeover.pointer {
                        let location = self.niri.seat.get_pointer().unwrap().current_location();
                        self.agent_user_motion(under, location, time);
                    }
                }
                None => {
                    // Clicking elsewhere closes menus open in the window, like for the user's
                    // own apps.
                    if let Some(window) = self.agent().takeover.window.clone() {
                        if self.agent_popup_grab_on(&window) {
                            if let Some((_, grab)) = &mut self.agent().popup_grab {
                                grab.ungrab(smithay::desktop::PopupUngrabStrategy::All);
                            }
                        }
                    }
                }
            }
        }

        if !self.agent().takeover.pointer {
            return;
        }
        let seat = self.agent_seat();
        let pointer = seat.get_pointer().unwrap();
        pointer.button(
            self,
            &ButtonEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time,
                button,
                state,
            },
        );
        pointer.frame(self);
    }

    /// Passes the user's scrolling on to the agent app under the pointer.
    pub fn agent_user_axis(&mut self, frame: AxisFrame) {
        if !self.agent_user_drives_pointer() {
            return;
        }
        let seat = self.agent_seat();
        let pointer = seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// Passes a key that the user's keyboard sent to its focus on to the agent app, when the
    /// focus is one of its windows.
    pub fn agent_user_key(&mut self, code: Keycode, state: KeyState, time: InputTime) {
        self.agent_pass_user_key(code, state, time, false);
    }

    /// Passes a key that the user's input method didn't use on to the agent app, when the user's
    /// keyboard focus is one of its windows.
    pub fn agent_user_ime_key(&mut self, code: Keycode, state: KeyState, time: InputTime) {
        self.agent_pass_user_key(code, state, time, true);
    }

    fn agent_pass_user_key(
        &mut self,
        code: Keycode,
        state: KeyState,
        time: InputTime,
        from_input_method: bool,
    ) {
        if self.niri.agent.is_none() {
            return;
        }

        // Agent apps share the user's input method (see AgentState::new). While it holds the
        // keyboard, keys go to it first, and it sends the ones it doesn't use back.
        let to_input_method =
            !from_input_method && self.niri.seat.input_method().keyboard_grabbed();

        if state == KeyState::Pressed {
            let focus = self.niri.seat.get_keyboard().unwrap().current_focus();
            let Some(target) = focus.and_then(|focus| self.agent_user_target(&focus)) else {
                return;
            };
            self.agent_start_takeover(target);
            if to_input_method {
                return;
            }
            self.agent().takeover.keys.push(code);
        } else {
            let keys = &mut self.agent().takeover.keys;
            let Some(idx) = keys.iter().position(|key| *key == code) else {
                return;
            };
            keys.remove(idx);
        }

        let seat = self.agent_seat();
        let keyboard = seat.get_keyboard().unwrap();
        keyboard.input(
            self,
            code,
            state,
            SERIAL_COUNTER.next_serial(),
            time,
            |_, _, _| FilterResult::<()>::Forward,
        );
    }

    /// Syncs a selection that a client set to the other seat during a takeover.
    pub fn agent_new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        seat: &Seat<State>,
    ) {
        let Some(agent) = &mut self.niri.agent else {
            return;
        };
        let side = if agent.is_agent_seat(seat) {
            Side::Agent
        } else if *seat == self.niri.seat {
            Side::User
        } else {
            return;
        };

        let takeover = &mut agent.takeover;
        takeover
            .selections
            .retain(|(s, t, _)| !(*s == side && *t == ty));
        let mimes = source.map(|source| source.mime_types());
        if let Some(mimes) = &mimes {
            takeover.selections.push((side, ty, mimes.clone()));
        }
        // The client's selection replaced the mirror on this seat.
        takeover
            .mirrors
            .retain(|mirror| !(mirror.on == side && mirror.ty == ty));

        if takeover.window.is_some() {
            match mimes {
                Some(mimes) => self.agent_mirror_selection(side.other(), ty, mimes),
                None => self.agent_clear_mirror(side.other(), ty),
            }
        } else if side == Side::Agent {
            // What the user copied in an agent app stays theirs until the agent copies something
            // else; then it goes away rather than turning into what the agent copied.
            self.agent_clear_mirror(Side::User, ty);
        }
    }

    fn agent_seat_of(&mut self, side: Side) -> Seat<State> {
        match side {
            Side::User => self.niri.seat.clone(),
            Side::Agent => self.agent_seat(),
        }
    }

    /// Sets a selection on the seat on `on` that passes on the other seat's.
    fn agent_mirror_selection(&mut self, on: Side, ty: SelectionTarget, mimes: Vec<String>) {
        // Only recognized by its address.
        let token: Arc<[u8]> = Arc::from(Vec::new());
        let seat = self.agent_seat_of(on);
        let dh = &self.niri.display_handle;
        match ty {
            SelectionTarget::Clipboard => {
                set_data_device_selection(dh, &seat, mimes, token.clone())
            }
            SelectionTarget::Primary => set_primary_selection(dh, &seat, mimes, token.clone()),
        }

        let mirrors = &mut self.agent().takeover.mirrors;
        mirrors.retain(|mirror| !(mirror.on == on && mirror.ty == ty));
        mirrors.push(Mirror { token, on, ty });
    }

    /// Clears the selection on the seat on `on` if it's a mirror.
    fn agent_clear_mirror(&mut self, on: Side, ty: SelectionTarget) {
        let mirrors = &mut self.agent().takeover.mirrors;
        let Some(idx) = mirrors
            .iter()
            .position(|mirror| mirror.on == on && mirror.ty == ty)
        else {
            return;
        };
        mirrors.remove(idx);

        let seat = self.agent_seat_of(on);
        let dh = &self.niri.display_handle;
        match ty {
            SelectionTarget::Clipboard => clear_data_device_selection(dh, &seat),
            SelectionTarget::Primary => clear_primary_selection(dh, &seat),
        }
    }

    /// Passes a request for a mirror selection on to the other seat's selection.
    ///
    /// Gives `fd` back if `user_data` isn't a mirror.
    pub fn agent_send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        user_data: &Arc<[u8]>,
    ) -> Option<OwnedFd> {
        let Some(agent) = &self.niri.agent else {
            return Some(fd);
        };
        let Some(mirror) = agent
            .takeover
            .mirrors
            .iter()
            .find(|mirror| Arc::ptr_eq(&mirror.token, user_data))
        else {
            return Some(fd);
        };

        let from = self.agent_seat_of(mirror.on.other());
        let result = match ty {
            SelectionTarget::Clipboard => {
                request_data_device_client_selection(&from, mime_type, fd)
                    .map_err(|e| e.to_string())
            }
            SelectionTarget::Primary => {
                request_primary_client_selection(&from, mime_type, fd).map_err(|e| e.to_string())
            }
        };
        if let Err(err) = result {
            debug!("error passing on a selection between seats: {err}");
        }
        None
    }
}
