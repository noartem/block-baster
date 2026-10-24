use crate::game::board::SIZE;
use crate::game::generator::TRAY_SIZE;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slot {
    /// Centre of the slot.
    pub cx: f32,
    pub cy: f32,
    /// Hit area.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    /// Size of the whole screen in logical pixels.
    pub width: f32,
    pub height: f32,
    /// Top of the play area (below the HUD).
    pub top: f32,
    pub cell: f32,
    pub board_x: f32,
    pub board_y: f32,
    pub board_size: f32,
    /// Size of one block when a piece rests in the tray.
    pub tray_cell: f32,
    pub slots: [Slot; TRAY_SIZE],
}

/// Computes positions of the board and tray for a play area of the given size.
pub fn compute_layout(width: f32, height: f32) -> Layout {
    compute_layout_in(0.0, width, height)
}

/// Like compute_layout, for a play area that starts `top` pixels down a screen of the given size.
pub fn compute_layout_in(top: f32, width: f32, screen_height: f32) -> Layout {
    let height = (screen_height - top).max(1.0);
    let pad = (width.min(height) * 0.035).max(10.0);
    let gap = (height * 0.03).max(12.0);
    // The tray needs roughly 3.3 board cells of height to show 5-long pieces at ~60% scale.
    let by_width = width - pad * 2.0;
    let by_height = (height - pad * 2.0 - gap) / (1.0 + 3.3 / SIZE as f32);
    let board_size = ((by_width.min(by_height).min(640.0) / SIZE as f32).floor() * SIZE as f32).max(SIZE as f32 * 8.0);
    let cell = board_size / SIZE as f32;
    let tray_h = cell * 3.3;
    let total = board_size + gap + tray_h;
    let board_x = ((width - board_size) / 2.0).round();
    let board_y = top + (pad.max((height - total) / 2.0 - height * 0.02)).round();
    let tray_top = board_y + board_size + gap;
    let slot_w = (width / TRAY_SIZE as f32).min(board_size * 1.12 / TRAY_SIZE as f32);
    let tray_left = (width - slot_w * TRAY_SIZE as f32) / 2.0;
    let tray_cell = (cell * 0.6).min(slot_w * 0.88 / 5.0);
    let mut slots = [Slot::default(); TRAY_SIZE];
    for (i, slot) in slots.iter_mut().enumerate() {
        let x = tray_left + slot_w * i as f32;
        // Hit areas reach the screen edges and bottom, so small pieces are easy to grab.
        let left = if i == 0 { 0.0 } else { x };
        let right = if i == TRAY_SIZE - 1 { width } else { x + slot_w };
        let y = tray_top - gap / 2.0;
        *slot = Slot { cx: x + slot_w / 2.0, cy: tray_top + tray_h / 2.0, x: left, y, w: right - left, h: (tray_h + gap).max(screen_height - y) };
    }
    Layout { width, height: screen_height, top, cell, board_x, board_y, board_size, tray_cell, slots }
}

impl Layout {
    /// Index of the tray slot under the point.
    pub fn slot_at(&self, x: f32, y: f32) -> Option<usize> {
        self.slots.iter().position(|s| x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h)
    }

    /// Top-left corner and block size of a piece of the given size resting in a tray slot.
    pub fn tray_piece_position(&self, slot: usize, rows: i32, cols: i32) -> (f32, f32, f32) {
        let s = self.slots[slot];
        let cell = self.tray_cell;
        (s.cx - cols as f32 * cell / 2.0, s.cy - rows as f32 * cell / 2.0, cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_board_and_tray_inside_common_screens() {
        for (w, h) in [(360.0, 640.0), (390.0, 760.0), (412.0, 870.0), (768.0, 1000.0), (1280.0, 720.0), (320.0, 480.0)] {
            let l = compute_layout(w, h);
            assert_eq!(l.board_size % 8.0, 0.0);
            assert!(l.board_x >= 0.0);
            assert!(l.board_x + l.board_size <= w);
            assert!(l.board_y >= 0.0);
            for s in &l.slots {
                assert!(s.cy > l.board_y + l.board_size);
                assert!(s.cy + l.tray_cell * 2.5 <= h);
                assert!(l.tray_cell * 5.0 <= s.w);
            }
            assert!(l.slots[0].x + l.slots[0].w <= l.slots[1].x + 0.001);
        }
    }

    #[test]
    fn hit_tests_tray_slots() {
        let l = compute_layout(390.0, 760.0);
        assert_eq!(l.slot_at(l.slots[1].cx, l.slots[1].cy), Some(1));
        assert_eq!(l.slot_at(l.board_x + 5.0, l.board_y + 5.0), None);
    }

    #[test]
    fn shifts_everything_below_the_hud() {
        let a = compute_layout(390.0, 700.0);
        let b = compute_layout_in(100.0, 390.0, 800.0);
        assert_eq!(b.board_y, a.board_y + 100.0);
        assert_eq!(b.cell, a.cell);
        assert_eq!(b.slots[0].cy, a.slots[0].cy + 100.0);
        assert_eq!(b.slots[0].y + b.slots[0].h, 800.0);
    }
}
