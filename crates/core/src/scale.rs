//! How the layout's y becomes a height on screen.

/// Maps the layout's y (0 at the top of the document, 1 at the end of
/// the longest version) to a fraction of the plot's height.
///
/// Two independent choices make it up: how much of the document's
/// length the plot's height stands for (`top`), and whether length is
/// drawn in proportion or on a logarithmic scale (`log`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YScale {
    /// The layout y drawn at the bottom of the plot: 1 to fit the longest
    /// version of the whole history, less to fit only the versions on
    /// screen. Anything beyond it is off the bottom.
    pub top: f32,
    /// 0 for a linear scale. Otherwise the scale is logarithmic and this
    /// is its steepness: y is drawn at `ln(1 + log * y)`, so the first
    /// `1 / log` of the document is drawn nearly in proportion and
    /// everything after it is progressively squeezed.
    pub log: f32,
}

impl YScale {
    /// The whole history, in proportion.
    pub const WHOLE: YScale = YScale { top: 1.0, log: 0.0 };

    /// The steepness of a logarithmic scale suited to versions of the
    /// given lengths (in the layout's y): the scale is close to linear
    /// over the first eighth of a typical (median) version. The start of
    /// the document stays readable, and a version many times the usual
    /// length takes up only a few times the usual height.
    pub fn log_for(extents: &[f32]) -> f32 {
        let mut lengths: Vec<f32> = extents.iter().copied().filter(|&e| e > 0.0).collect();
        if lengths.is_empty() {
            return 0.0;
        }
        lengths.sort_by(f32::total_cmp);
        8.0 / lengths[lengths.len() / 2]
    }

    /// One step of easing the scale toward `target`, `elapsed`
    /// milliseconds after the last: the top closes a fixed share of the
    /// remaining gap per unit of time, in proportion (halving takes as
    /// long as doubling), and lands exactly once it is within a
    /// thousandth. The steepness is not eased; it takes `target`'s.
    pub fn eased_toward(self, target: YScale, elapsed: f32) -> YScale {
        /// Milliseconds for the gap to shrink by a factor of e.
        const PACE: f32 = 90.0;
        let gap = (target.top / self.top).ln();
        let remaining = gap * (-elapsed.max(0.0) / PACE).exp();
        let top = if remaining.abs() < 1e-3 || !remaining.is_finite() {
            target.top
        } else {
            target.top / remaining.exp()
        };
        YScale { top, ..target }
    }

    fn curve(self, y: f32) -> f32 {
        if self.log > 0.0 {
            (self.log * y).ln_1p()
        } else {
            y
        }
    }

    /// How far down the plot a layout y is drawn: 0 at the top, 1 at the
    /// bottom, more if it is off the bottom.
    pub fn down(self, y: f32) -> f32 {
        self.curve(y) / self.curve(self.top)
    }

    /// The layout y drawn `down` of the way down the plot.
    pub fn at(self, down: f32) -> f32 {
        let curved = down * self.curve(self.top);
        if self.log > 0.0 {
            curved.exp_m1() / self.log
        } else {
            curved
        }
    }
}

impl Default for YScale {
    fn default() -> YScale {
        YScale::WHOLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn the_whole_linear_scale_changes_nothing() {
        for y in [0.0, 0.25, 1.0] {
            assert_eq!(YScale::WHOLE.down(y), y);
            assert_eq!(YScale::WHOLE.at(y), y);
        }
    }

    #[test]
    fn a_lower_top_stretches_what_is_on_screen_to_fill_the_plot() {
        let scale = YScale {
            top: 0.25,
            log: 0.0,
        };
        assert!(close(scale.down(0.25), 1.0));
        assert!(close(scale.down(0.1), 0.4));
        // A longer version runs off the bottom.
        assert!(scale.down(0.5) > 1.0);
    }

    #[test]
    fn a_log_scale_keeps_the_ends_and_gives_the_start_more_room() {
        let scale = YScale {
            top: 1.0,
            log: 80.0,
        };
        assert_eq!(scale.down(0.0), 0.0);
        assert!(close(scale.down(1.0), 1.0));
        // The first tenth of the document gets half the height…
        assert!(close(scale.down(0.1), 0.5));
        // …and each further stretch of equal length gets less.
        let steps: Vec<f32> = (0..=10).map(|i| scale.down(i as f32 / 10.0)).collect();
        assert!(steps.windows(3).all(|w| w[1] - w[0] > w[2] - w[1]));
    }

    #[test]
    fn at_undoes_down_on_every_scale() {
        for scale in [
            YScale::WHOLE,
            YScale { top: 0.3, log: 0.0 },
            YScale {
                top: 1.0,
                log: 40.0,
            },
            YScale {
                top: 0.3,
                log: 40.0,
            },
        ] {
            for y in [0.0, 0.01, 0.2, 0.3] {
                assert!(close(scale.at(scale.down(y)), y), "{scale:?} {y}");
            }
            assert!(close(scale.down(scale.top), 1.0));
        }
    }

    #[test]
    fn easing_closes_the_gap_in_proportion_and_then_lands() {
        let from = YScale { top: 1.0, log: 0.0 };
        let to = YScale {
            top: 0.25,
            log: 3.0,
        };
        // No time, no movement — but the steepness is the target's.
        assert_eq!(from.eased_toward(to, 0.0), YScale { top: 1.0, log: 3.0 });
        // Each step covers the same share of what is left, in proportion.
        let once = from.eased_toward(to, 50.0);
        let twice = once.eased_toward(to, 50.0);
        assert!(once.top < 1.0 && twice.top < once.top && twice.top > 0.25);
        assert!(close(twice.top, from.eased_toward(to, 100.0).top));
        // Growing mirrors shrinking.
        let back = to.eased_toward(from, 50.0);
        assert!(close(back.top * once.top, 0.25));
        // Given long enough it arrives exactly, and stays.
        let there = from.eased_toward(to, 5000.0);
        assert_eq!(there.top, 0.25);
        assert_eq!(there.eased_toward(to, 16.0), there);
    }

    #[test]
    fn the_log_steepness_follows_the_typical_version_not_the_longest() {
        // Nine ordinary versions and one enormous one.
        let mut extents = vec![0.05; 9];
        extents.push(1.0);
        let log = YScale::log_for(&extents);
        assert!(close(log, 160.0));
        let scale = YScale { top: 1.0, log };
        // An ordinary version gets 43% of the height, not 5%.
        assert!(close(scale.down(0.05), 9f32.ln() / 161f32.ln()));
        // Empty versions do not count, and no versions means linear.
        assert!(close(YScale::log_for(&[0.0, 0.0, 0.5]), 16.0));
        assert_eq!(YScale::log_for(&[]), 0.0);
    }
}
