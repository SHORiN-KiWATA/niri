use std::rc::Rc;

use smithay::utils::{Logical, Point, Rectangle, Size};

use super::{Animation, Clock, LayoutElement, Options, OverviewProgress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug)]
pub struct GridOverview<W: LayoutElement> {
    pub open: bool,
    pub(super) progress: Option<OverviewProgress>,
    pub layout: GridLayout<W>,
    pub focus: (usize, usize),
    pub saved_active_window_id: Option<W::Id>,
    pub saved_view_offset: f64,
    pub entry_positions: Vec<(W::Id, Point<f64, Logical>)>,
    pub entry_scales: Vec<(W::Id, f64)>,
    pub rearrange_anim: Option<Animation>,
    pub clock: Clock,
    pub options: Rc<Options>,
}

impl<W: LayoutElement> GridOverview<W> {
    pub fn new(clock: Clock, options: Rc<Options>) -> Self {
        Self {
            open: false,
            progress: None,
            layout: GridLayout::empty(),
            focus: (0, 0),
            saved_active_window_id: None,
            saved_view_offset: 0.,
            entry_positions: Vec::new(),
            entry_scales: Vec::new(),
            rearrange_anim: None,
            clock,
            options,
        }
    }

    pub fn is_fully_open(&self) -> bool {
        self.open && matches!(self.progress, Some(OverviewProgress::Open))
    }

    pub fn progress_value(&self) -> f64 {
        self.progress.as_ref().map_or(0., |p| p.value())
    }

    pub fn is_animation(&self) -> bool {
        self.progress.as_ref().map_or(false, |p| p.is_animation()) || self.rearrange_anim.is_some()
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.rearrange_anim = None;

        let from = self.progress.take().map_or(0., |p| p.value());
        let to = if self.open { 1. } else { 0. };

        self.progress = Some(OverviewProgress::Animation(Animation::new(
            self.clock.clone(),
            from,
            to,
            0.,
            self.options.animations.grid_overview_open_close.0,
        )));
    }

    pub fn compute_layout(
        &mut self,
        tiles: &[(W::Id, Size<f64, Logical>)],
        area: Rectangle<f64, Logical>,
    ) {
        let old_layout = self.layout.clone();
        if self.is_fully_open() && !self.layout.tiles.is_empty() {
            self.rearrange_anim = Some(Animation::new(
                self.clock.clone(),
                0.,
                1.,
                0.,
                self.options.animations.window_movement.0,
            ));
        }
        self.layout = GridLayout::compute(tiles, area, &self.options.grid_overview);
        let mut new_entries = Vec::new();
        for (id, info) in &self.layout.tiles {
            let pos = self
                .entry_positions
                .iter()
                .find(|(eid, _)| eid == id)
                .map(|(_, pos)| *pos)
                .or_else(|| {
                    old_layout
                        .tiles
                        .iter()
                        .find(|(eid, _)| eid == id)
                        .map(|(_, old_info)| old_info.target_pos)
                })
                .unwrap_or(info.target_pos);
            new_entries.push((id.clone(), pos));
        }
        self.entry_positions = new_entries;
        let mut new_scales = Vec::new();
        for (id, info) in &self.layout.tiles {
            let scale = self
                .entry_scales
                .iter()
                .find(|(eid, _)| eid == id)
                .map(|(_, s)| *s)
                .or_else(|| {
                    old_layout
                        .tiles
                        .iter()
                        .find(|(eid, _)| eid == id)
                        .map(|(_, old_info)| old_info.target_scale)
                })
                .unwrap_or(info.target_scale);
            new_scales.push((id.clone(), scale));
        }
        self.entry_scales = new_scales;
        if self.layout.tiles.is_empty() {
            self.focus = (0, 0);
        } else {
            self.focus.0 = self.focus.0.min(self.layout.rows.saturating_sub(1));
            self.focus.1 = self.focus.1.min(self.layout.cols.saturating_sub(1));
        }
    }

    pub fn navigate(&mut self, dir: GridDirection) {
        if self.layout.tiles.is_empty() {
            return;
        }
        let (row, col) = self.focus;
        self.focus = match dir {
            GridDirection::Up => {
                if row > 0 {
                    (row - 1, col)
                } else {
                    (row, col)
                }
            }
            GridDirection::Down => {
                if row + 1 < self.layout.rows {
                    (row + 1, col)
                } else {
                    (row, col)
                }
            }
            GridDirection::Left => {
                if col > 0 {
                    (row, col - 1)
                } else {
                    (row, col)
                }
            }
            GridDirection::Right => {
                if col + 1 < self.layout.cols {
                    (row, col + 1)
                } else {
                    (row, col)
                }
            }
        };
        let idx = self.focus.0 * self.layout.cols + self.focus.1;
        if idx >= self.layout.tiles.len() {
            let last_idx = self.layout.tiles.len().saturating_sub(1);
            self.focus = (last_idx / self.layout.cols, last_idx % self.layout.cols);
        }
    }

    pub fn focused_id(&self) -> Option<W::Id> {
        let idx = self.focus.0 * self.layout.cols + self.focus.1;
        self.layout.tiles.get(idx).map(|(id, _)| id.clone())
    }

    pub fn find_grid_info(&self, id: &W::Id) -> Option<&GridTileInfo> {
        self.layout
            .tiles
            .iter()
            .find_map(|(tid, info)| if tid == id { Some(info) } else { None })
    }

    pub fn find_grid_index(&self, id: &W::Id) -> Option<(usize, usize)> {
        self.layout
            .tiles
            .iter()
            .enumerate()
            .find_map(|(idx, (tid, _))| {
                if tid == id {
                    Some((idx / self.layout.cols, idx % self.layout.cols))
                } else {
                    None
                }
            })
    }

    pub fn advance_animations(&mut self) {
        if let Some(OverviewProgress::Animation(anim)) = &mut self.progress {
            if anim.is_done() {
                self.progress = if self.open {
                    Some(OverviewProgress::Open)
                } else {
                    None
                };
            }
        }
        if let Some(anim) = &mut self.rearrange_anim {
            if anim.is_done() {
                self.rearrange_anim = None;
                for (id, info) in &self.layout.tiles {
                    if let Some(entry) = self.entry_positions.iter_mut().find(|(eid, _)| eid == id)
                    {
                        entry.1 = info.target_pos;
                    }
                    if let Some(entry) = self.entry_scales.iter_mut().find(|(eid, _)| eid == id) {
                        entry.1 = info.target_scale;
                    }
                }
            }
        }
    }

    pub fn are_animations_ongoing(&self) -> bool {
        self.progress.as_ref().map_or(false, |p| p.is_animation()) || self.rearrange_anim.is_some()
    }
}

#[derive(Debug)]
pub struct GridLayout<W: LayoutElement> {
    pub cols: usize,
    pub rows: usize,
    pub gap: f64,
    pub tiles: Vec<(W::Id, GridTileInfo)>,
}

impl<W: LayoutElement> Clone for GridLayout<W> {
    fn clone(&self) -> Self {
        Self {
            cols: self.cols,
            rows: self.rows,
            gap: self.gap,
            tiles: self.tiles.clone(),
        }
    }
}

impl<W: LayoutElement> GridLayout<W> {
    pub fn empty() -> Self {
        Self {
            cols: 0,
            rows: 0,
            gap: 0.,
            tiles: Vec::new(),
        }
    }

    pub fn compute(
        tiles: &[(W::Id, Size<f64, Logical>)],
        area: Rectangle<f64, Logical>,
        config: &niri_config::GridOverview,
    ) -> Self {
        let n = tiles.len();
        if n == 0 {
            return Self::empty();
        }

        let gap = config.gap;
        let padding = config.padding;

        let content_w = (area.size.w - padding * 2.).max(1.);
        let content_h = (area.size.h - padding * 2.).max(1.);
        let aspect = content_w / content_h;

        let mut cols = ((n as f64 * aspect).sqrt()).ceil().max(1.) as usize;
        let mut rows = n.div_ceil(cols);
        if (cols - 1) * rows >= n && cols > 1 {
            cols -= 1;
            rows = n.div_ceil(cols);
        }

        let cell_w = (content_w - gap * (cols as f64 - 1.)) / cols as f64;
        let cell_h = (content_h - gap * (rows as f64 - 1.)) / rows as f64;
        let cell_ar = cell_w / cell_h;

        let mut out_tiles = Vec::with_capacity(n);

        for (idx, (id, in_size)) in tiles.iter().enumerate() {
            let row = idx / cols;
            let col = idx % cols;

            let tile_ar = in_size.w / in_size.h.max(1.);

            let (scaled_w, scaled_h) = if tile_ar > cell_ar {
                let sw = cell_w;
                let sh = sw / tile_ar;
                (sw, sh.max(cell_h * config.min_scale))
            } else {
                let sh = cell_h;
                let sw = sh * tile_ar;
                (sw.max(cell_w * config.min_scale), sh)
            };

            let target_scale = scaled_w / in_size.w.max(1.);

            let cell_x = padding + col as f64 * (cell_w + gap);
            let cell_y = padding + row as f64 * (cell_h + gap);

            let target_pos = Point::from((
                area.loc.x + cell_x + (cell_w - scaled_w) / 2.,
                area.loc.y + cell_y + (cell_h - scaled_h) / 2.,
            ));

            let target_size = Size::from((scaled_w, scaled_h));

            out_tiles.push((
                id.clone(),
                GridTileInfo {
                    row,
                    col,
                    target_pos,
                    target_size,
                    target_scale,
                },
            ));
        }

        if !out_tiles.is_empty() {
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            for (_, info) in &out_tiles {
                min_x = min_x.min(info.target_pos.x);
                min_y = min_y.min(info.target_pos.y);
                max_x = max_x.max(info.target_pos.x + info.target_size.w);
                max_y = max_y.max(info.target_pos.y + info.target_size.h);
            }
            let grid_w = max_x - min_x;
            let grid_h = max_y - min_y;
            let offset_x = area.loc.x + (content_w + padding * 2. - grid_w) / 2. - min_x;
            let offset_y = area.loc.y + (content_h + padding * 2. - grid_h) / 2. - min_y;
            for (_, info) in &mut out_tiles {
                info.target_pos.x += offset_x;
                info.target_pos.y += offset_y;
            }
        }

        Self {
            cols,
            rows,
            gap,
            tiles: out_tiles,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridTileInfo {
    pub row: usize,
    pub col: usize,
    pub target_pos: Point<f64, Logical>,
    pub target_size: Size<f64, Logical>,
    pub target_scale: f64,
}
