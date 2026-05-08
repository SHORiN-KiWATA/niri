use crate::appearance::{Color, WorkspaceShadow, WorkspaceShadowPart, DEFAULT_BACKDROP_COLOR};
use crate::utils::{Flag, MergeWith};
use crate::FloatOrInt;

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct SpawnAtStartup {
    #[knuffel(arguments)]
    pub command: Vec<String>,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct SpawnShAtStartup {
    #[knuffel(argument)]
    pub command: String,
}

#[derive(Debug, PartialEq)]
pub struct Cursor {
    pub xcursor_theme: String,
    pub xcursor_size: u8,
    pub hide_when_typing: bool,
    pub hide_after_inactive_ms: Option<u32>,
    pub shake_to_enlarge: Option<ShakeToEnlarge>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShakeToEnlarge {
    pub off: bool,
    pub zoom_factor: f64,
    pub hold_duration_ms: u32,
    pub threshold: f64,
    pub grow: bool,
    pub grow_speed: f64,
}

impl Default for ShakeToEnlarge {
    fn default() -> Self {
        Self {
            off: false,
            zoom_factor: 3.0,
            hold_duration_ms: 1200,
            threshold: 2000.0,
            grow: false,
            grow_speed: 0.01,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub struct ShakeToEnlargePart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child)]
    pub on: bool,
    #[knuffel(child, unwrap(argument))]
    pub zoom_factor: Option<FloatOrInt<0, { i32::MAX }>>,
    #[knuffel(child, unwrap(argument))]
    pub hold_duration_ms: Option<u32>,
    #[knuffel(child, unwrap(argument))]
    pub threshold: Option<FloatOrInt<0, { i32::MAX }>>,
    #[knuffel(child)]
    pub grow: Option<Flag>,
    #[knuffel(child, unwrap(argument))]
    pub grow_speed: Option<FloatOrInt<0, { i32::MAX }>>,
}

impl MergeWith<ShakeToEnlargePart> for ShakeToEnlarge {
    fn merge_with(&mut self, part: &ShakeToEnlargePart) {
        self.off |= part.off;
        if part.on {
            self.off = false;
        }

        merge!((self, part), zoom_factor, threshold, grow_speed, grow);
        merge_clone!((self, part), hold_duration_ms);
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            xcursor_theme: String::from("default"),
            xcursor_size: 24,
            hide_when_typing: false,
            hide_after_inactive_ms: None,
            shake_to_enlarge: Some(ShakeToEnlarge::default()),
        }
    }
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct CursorPart {
    #[knuffel(child, unwrap(argument))]
    pub xcursor_theme: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub xcursor_size: Option<u8>,
    #[knuffel(child)]
    pub hide_when_typing: Option<Flag>,
    #[knuffel(child, unwrap(argument))]
    pub hide_after_inactive_ms: Option<u32>,
    #[knuffel(child)]
    pub shake_to_enlarge: Option<ShakeToEnlargePart>,
}

impl MergeWith<CursorPart> for Cursor {
    fn merge_with(&mut self, part: &CursorPart) {
        merge_clone!((self, part), xcursor_theme, xcursor_size);
        merge!((self, part), hide_when_typing);
        merge_clone_opt!((self, part), hide_after_inactive_ms);
        if let Some(x) = &part.shake_to_enlarge {
            if let Some(s) = &mut self.shake_to_enlarge {
                s.merge_with(x);
            } else {
                self.shake_to_enlarge = Some(ShakeToEnlarge::from_part(x));
            }
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub struct ScreenshotPath(#[knuffel(argument)] pub Option<String>);

impl Default for ScreenshotPath {
    fn default() -> Self {
        Self(Some(String::from(
            "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png",
        )))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyOverlay {
    pub skip_at_startup: bool,
    pub hide_not_bound: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyOverlayPart {
    #[knuffel(child)]
    pub skip_at_startup: Option<Flag>,
    #[knuffel(child)]
    pub hide_not_bound: Option<Flag>,
}

impl MergeWith<HotkeyOverlayPart> for HotkeyOverlay {
    fn merge_with(&mut self, part: &HotkeyOverlayPart) {
        merge!((self, part), skip_at_startup, hide_not_bound);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigNotification {
    pub disable_failed: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigNotificationPart {
    #[knuffel(child)]
    pub disable_failed: Option<Flag>,
}

impl MergeWith<ConfigNotificationPart> for ConfigNotification {
    fn merge_with(&mut self, part: &ConfigNotificationPart) {
        merge!((self, part), disable_failed);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Clipboard {
    pub disable_primary: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardPart {
    #[knuffel(child)]
    pub disable_primary: Option<Flag>,
}

impl MergeWith<ClipboardPart> for Clipboard {
    fn merge_with(&mut self, part: &ClipboardPart) {
        merge!((self, part), disable_primary);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Magnifier {
    pub off: bool,
    pub zoom_factor: f64,
    pub track_cursor: bool,
    pub scale_cursor: bool,
}

impl Default for Magnifier {
    fn default() -> Self {
        Self {
            off: false,
            zoom_factor: 2.0,
            track_cursor: true,
            scale_cursor: true,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub struct MagnifierPart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child)]
    pub on: bool,
    #[knuffel(child, unwrap(argument))]
    pub zoom_factor: Option<FloatOrInt<0, { i32::MAX }>>,
    #[knuffel(child)]
    pub track_cursor: Option<Flag>,
    #[knuffel(child)]
    pub scale_cursor: Option<Flag>,
}

impl MergeWith<MagnifierPart> for Magnifier {
    fn merge_with(&mut self, part: &MagnifierPart) {
        self.off |= part.off;
        if part.on {
            self.off = false;
        }
        merge!((self, part), zoom_factor, track_cursor, scale_cursor);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overview {
    pub zoom: f64,
    pub backdrop_color: Color,
    pub workspace_shadow: WorkspaceShadow,
}

impl Default for Overview {
    fn default() -> Self {
        Self {
            zoom: 0.5,
            backdrop_color: DEFAULT_BACKDROP_COLOR,
            workspace_shadow: WorkspaceShadow::default(),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq)]
pub struct OverviewPart {
    #[knuffel(child, unwrap(argument))]
    pub zoom: Option<FloatOrInt<0, 1>>,
    #[knuffel(child)]
    pub backdrop_color: Option<Color>,
    #[knuffel(child)]
    pub workspace_shadow: Option<WorkspaceShadowPart>,
}

impl MergeWith<OverviewPart> for Overview {
    fn merge_with(&mut self, part: &OverviewPart) {
        merge!((self, part), zoom, workspace_shadow);
        merge_clone!((self, part), backdrop_color);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridOverview {
    pub gap: f64,
    pub padding: f64,
    pub backdrop_color: Color,
    pub min_scale: f64,
    pub focused_window_scale: f64,
}

impl Default for GridOverview {
    fn default() -> Self {
        Self {
            gap: 8.,
            padding: 16.,
            backdrop_color: DEFAULT_BACKDROP_COLOR,
            min_scale: 0.08,
            focused_window_scale: 1.04,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq)]
pub struct GridOverviewPart {
    #[knuffel(child, unwrap(argument))]
    pub gap: Option<f64>,
    #[knuffel(child, unwrap(argument))]
    pub padding: Option<f64>,
    #[knuffel(child)]
    pub backdrop_color: Option<Color>,
    #[knuffel(child, unwrap(argument))]
    pub min_scale: Option<FloatOrInt<0, 1>>,
    #[knuffel(child, unwrap(argument))]
    pub focused_window_scale: Option<FloatOrInt<1, 2>>,
}

impl MergeWith<GridOverviewPart> for GridOverview {
    fn merge_with(&mut self, part: &GridOverviewPart) {
        if let Some(gap) = part.gap {
            self.gap = gap;
        }
        if let Some(padding) = part.padding {
            self.padding = padding;
        }
        merge_clone!((self, part), backdrop_color);
        if let Some(min_scale) = &part.min_scale {
            self.min_scale = min_scale.0;
        }
        if let Some(focused_window_scale) = &part.focused_window_scale {
            self.focused_window_scale = focused_window_scale.0;
        }
    }
}

#[derive(knuffel::Decode, Debug, Default, Clone, PartialEq, Eq)]
pub struct Environment(#[knuffel(children)] pub Vec<EnvironmentVariable>);

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentVariable {
    #[knuffel(node_name)]
    pub name: String,
    #[knuffel(argument)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XwaylandSatellite {
    pub off: bool,
    pub path: String,
}

impl Default for XwaylandSatellite {
    fn default() -> Self {
        Self {
            off: false,
            path: String::from("xwayland-satellite"),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct XwaylandSatellitePart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child)]
    pub on: bool,
    #[knuffel(child, unwrap(argument))]
    pub path: Option<String>,
}

impl MergeWith<XwaylandSatellitePart> for XwaylandSatellite {
    fn merge_with(&mut self, part: &XwaylandSatellitePart) {
        self.off |= part.off;
        if part.on {
            self.off = false;
        }

        merge_clone!((self, part), path);
    }
}
