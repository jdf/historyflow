//! Which part of the history is on screen.

/// A window onto the layout's x axis: `width` of it, starting at
/// `start`, fills the plot. Only x zooms; y always shows the whole
/// document.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub start: f32,
    pub width: f32,
}

impl View {
    /// The whole history.
    pub const FULL: View = View {
        start: 0.0,
        width: 1.0,
    };

    /// The narrowest a view of versions at `xs` may be: the least x that
    /// three consecutive versions span, so that at full zoom three
    /// revisions fill the plot. Where versions are unevenly spaced that
    /// is the three closest together (ignoring any at one x, which no
    /// zoom can separate). With fewer than four versions there is
    /// nothing to zoom into.
    pub fn min_width(xs: &[f32]) -> f32 {
        const FLOOR: f32 = 1e-5;
        if xs.len() < 4 {
            return 1.0;
        }
        let narrowest = xs
            .windows(3)
            .map(|three| three[2] - three[0])
            .filter(|&span| span > 0.0)
            .fold(1.0, f32::min);
        narrowest.max(FLOOR)
    }

    /// Where layout x falls across the plot: 0 at its left edge, 1 at
    /// its right.
    pub fn across(self, x: f32) -> f32 {
        (x - self.start) / self.width
    }

    /// The layout x at a fraction of the way across the plot.
    pub fn at(self, across: f32) -> f32 {
        self.start + across * self.width
    }

    /// Scales the width by `factor` (less than 1 zooms in), holding fixed
    /// whatever is `anchor` of the way across the plot, within the limits:
    /// no narrower than `min_width`, no wider than the whole history.
    pub fn zoom(self, factor: f32, anchor: f32, min_width: f32) -> View {
        let width = (self.width * factor).clamp(min_width.min(1.0), 1.0);
        let start = self.at(anchor) - anchor * width;
        View { start, width }.within_bounds()
    }

    /// Slides the view by `delta` of its own width; positive moves later
    /// in the history.
    pub fn pan(self, delta: f32) -> View {
        View {
            start: self.start + delta * self.width,
            ..self
        }
        .within_bounds()
    }

    /// Slides the view, if need be, just far enough that `x` is in it.
    pub fn reveal(self, x: f32) -> View {
        let start = self.start.min(x).max(x - self.width);
        View { start, ..self }.within_bounds()
    }

    /// A view of `width` starting at `start`, brought within the limits.
    pub fn new(start: f32, width: f32, min_width: f32) -> View {
        let width = width.clamp(min_width.min(1.0), 1.0);
        View { start, width }.within_bounds()
    }

    fn within_bounds(self) -> View {
        View {
            start: self.start.clamp(0.0, 1.0 - self.width),
            ..self
        }
    }
}

impl Default for View {
    fn default() -> View {
        View::FULL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn even(count: usize) -> Vec<f32> {
        (0..count).map(|i| i as f32 / (count - 1) as f32).collect()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn fully_zoomed_in_three_evenly_spaced_versions_fill_the_plot() {
        let xs = even(11);
        let min = View::min_width(&xs);
        assert!(close(min, 0.2));
        // However hard the zoom, centered on version 5.
        let view = View::FULL.zoom(1e-9, 0.5, min);
        assert!(close(view.width, 0.2));
        assert!(close(view.across(xs[4]), 0.0));
        assert!(close(view.across(xs[5]), 0.5));
        assert!(close(view.across(xs[6]), 1.0));
    }

    #[test]
    fn fully_zoomed_out_is_the_whole_history_and_no_more() {
        let view = View {
            start: 0.6,
            width: 0.2,
        }
        .zoom(1e9, 0.3, 0.2);
        assert_eq!(view, View::FULL);
    }

    #[test]
    fn zooming_holds_the_anchor_still() {
        let before = View {
            start: 0.2,
            width: 0.5,
        };
        let after = before.zoom(0.5, 0.25, 0.01);
        assert!(close(after.width, 0.25));
        assert!(close(after.at(0.25), before.at(0.25)));
    }

    #[test]
    fn zooming_out_near_an_edge_stays_inside_the_history() {
        let view = View {
            start: 0.0,
            width: 0.1,
        }
        .zoom(4.0, 0.9, 0.01);
        assert_eq!(
            view,
            View {
                start: 0.0,
                width: 0.4
            }
        );
        let view = View {
            start: 0.9,
            width: 0.1,
        }
        .zoom(4.0, 0.1, 0.01);
        assert!(close(view.start, 0.6) && close(view.width, 0.4));
    }

    #[test]
    fn panning_moves_by_a_share_of_the_view_and_stops_at_the_ends() {
        let view = View {
            start: 0.4,
            width: 0.2,
        };
        assert!(close(view.pan(0.5).start, 0.5));
        assert!(close(view.pan(-0.5).start, 0.3));
        assert!(close(view.pan(100.0).start, 0.8));
        assert_eq!(view.pan(-100.0).start, 0.0);
        assert_eq!(View::FULL.pan(0.5), View::FULL);
    }

    #[test]
    fn reveal_moves_only_as_far_as_needed() {
        let view = View {
            start: 0.4,
            width: 0.2,
        };
        assert_eq!(view.reveal(0.5), view);
        assert!(close(view.reveal(0.3).start, 0.3));
        assert!(close(view.reveal(0.9).start, 0.7));
    }

    #[test]
    fn min_width_uses_the_closest_three_and_ignores_coincident_versions() {
        assert!(close(
            View::min_width(&[0.0, 0.5, 0.5, 0.5, 0.6, 0.7, 1.0]),
            0.1
        ));
        // Too few versions to zoom.
        assert_eq!(View::min_width(&even(3)), 1.0);
        assert_eq!(View::min_width(&[]), 1.0);
        // All at one x.
        assert_eq!(View::min_width(&[0.5; 5]), 1.0);
    }
}
