//! Overlay anchor placement math.
//!
//! Defines the `OverlayAnchor` positioning helper and its `origin` geometry, which
//! places the fixed-size overlay canvas at the chosen corner of the active display,
//! fully inside it and inset by a margin.

use gpui::{Bounds, Pixels, Point, Size};

/// Where an overlay surface is pinned on the active display. Internal positioning
/// helper only (no longer a user setting): the rail uses `TopRight`, the pill
/// `BottomCenter`. `DECK_OVERLAY=0/1` remains the master on/off in `mod.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayAnchor {
    TopRight,
    BottomCenter,
}

impl OverlayAnchor {
    /// Top-left origin for a `canvas` of the given size within `display` visible bounds,
    /// inset by `margin`.
    ///
    /// The returned point is the window origin (top-left) such that the fixed-size
    /// `canvas` sits at the chosen anchor and stays fully inside `display`:
    ///
    /// - `TopRight`: pinned to the top-right corner — `margin` from the top and right.
    /// - `BottomCenter`: horizontally centered, sitting `margin` above the bottom
    ///   edge (so it clears the dock).
    pub fn origin(
        self,
        display: Bounds<Pixels>,
        canvas: Size<Pixels>,
        margin: Pixels,
    ) -> Point<Pixels> {
        match self {
            OverlayAnchor::TopRight => {
                let x = display.right() - canvas.width - margin;
                let y = display.top() + margin;
                gpui::point(x, y)
            }
            OverlayAnchor::BottomCenter => {
                // `Pixels` has `Mul<f32>` but no `Div<f32>`, so halve with `* 0.5`.
                let x = display.left() + (display.size.width - canvas.width) * 0.5;
                let y = display.bottom() - canvas.height - margin;
                gpui::point(x, y)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // `super::*` brings the module's `OverlayAnchor` plus the `Bounds`/`Pixels`/`Point`/`Size`
    // imports into scope; only the free constructors `px`/`size`/`point` need pulling in here.
    use super::*;
    use gpui::{point, px, size};

    /// A 1440x900 display anchored at the screen origin, the canonical fixture.
    fn display() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1440.0), px(900.0)),
        }
    }

    fn canvas() -> Size<Pixels> {
        size(px(460.0), px(160.0))
    }

    const MARGIN: f32 = 16.0;

    #[test]
    fn bottom_center_is_centered_and_above_bottom() {
        let origin = OverlayAnchor::BottomCenter.origin(display(), canvas(), px(MARGIN));

        // Horizontally centered: (1440 - 460) / 2 = 490.
        assert_eq!(origin.x, px(490.0));
        // `margin` above the bottom: 900 - 160 - 16 = 724.
        assert_eq!(origin.y, px(724.0));
    }

    #[test]
    fn top_right_is_inset_from_top_and_right() {
        let origin = OverlayAnchor::TopRight.origin(display(), canvas(), px(MARGIN));

        // Right edge inset: 1440 - 460 - 16 = 964.
        assert_eq!(origin.x, px(964.0));
        // `margin` from the top.
        assert_eq!(origin.y, px(MARGIN));
    }

    #[test]
    fn both_anchors_keep_canvas_fully_inside_display() {
        let d = display();
        let c = canvas();
        for anchor in [OverlayAnchor::TopRight, OverlayAnchor::BottomCenter] {
            let origin = anchor.origin(d, c, px(MARGIN));

            // Top-left stays inside the display top-left.
            assert!(origin.x >= d.left(), "{anchor:?}: x {:?} < left", origin.x);
            assert!(origin.y >= d.top(), "{anchor:?}: y {:?} < top", origin.y);

            // Bottom-right of the canvas stays inside the display bottom-right.
            assert!(
                origin.x + c.width <= d.right(),
                "{anchor:?}: right {:?} > {:?}",
                origin.x + c.width,
                d.right()
            );
            assert!(
                origin.y + c.height <= d.bottom(),
                "{anchor:?}: bottom {:?} > {:?}",
                origin.y + c.height,
                d.bottom()
            );
        }
    }

    #[test]
    fn origin_respects_nonzero_display_offset() {
        // A secondary display offset to the right and down; origin must shift with it.
        let d = Bounds {
            origin: point(px(100.0), px(50.0)),
            size: size(px(1440.0), px(900.0)),
        };
        let origin = OverlayAnchor::TopRight.origin(d, canvas(), px(MARGIN));

        // Right edge: (100 + 1440) - 460 - 16 = 1064; top: 50 + 16 = 66.
        assert_eq!(origin.x, px(1064.0));
        assert_eq!(origin.y, px(66.0));
    }
}
