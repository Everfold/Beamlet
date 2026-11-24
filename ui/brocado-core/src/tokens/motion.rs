//! Animation duration and cubic-bezier easing curve tokens shared across design languages.

use std::time::Duration;

/// Standardized motion duration steps (`50ms` to `1000ms`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MotionDuration {
    /// `50ms` micro state changes.
    Short1,
    /// `100ms` quick hover/press transitions.
    Short2,
    /// `150ms` subtle reveal.
    #[default]
    Short3,
    /// `200ms` small surface fade/enter.
    Short4,
    /// `250ms` compact transitions.
    Medium1,
    /// `300ms` standard panel/drawer transitions.
    Medium2,
    /// `350ms` medium expand/collapse.
    Medium3,
    /// `400ms` rich component choreography.
    Medium4,
    /// `450ms` page transitions.
    Long1,
    /// `500ms` expressive emphasized transition.
    Long2,
    /// `550ms` large sheet transition.
    Long3,
    /// `600ms` full-screen view switch.
    Long4,
    /// `700ms` ambient container sweep.
    ExtraLong1,
    /// `800ms` extended choreography.
    ExtraLong2,
    /// `900ms` progressive reveal.
    ExtraLong3,
    /// `1000ms` continuous cycle base.
    ExtraLong4,
}

impl MotionDuration {
    /// Returns the millisecond duration for this token.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        match self {
            Self::Short1 => 50,
            Self::Short2 => 100,
            Self::Short3 => 150,
            Self::Short4 => 200,
            Self::Medium1 => 250,
            Self::Medium2 => 300,
            Self::Medium3 => 350,
            Self::Medium4 => 400,
            Self::Long1 => 450,
            Self::Long2 => 500,
            Self::Long3 => 550,
            Self::Long4 => 600,
            Self::ExtraLong1 => 700,
            Self::ExtraLong2 => 800,
            Self::ExtraLong3 => 900,
            Self::ExtraLong4 => 1000,
        }
    }

    /// Converts this token into [`std::time::Duration`].
    #[must_use]
    pub const fn to_duration(self) -> Duration {
        Duration::from_millis(self.as_millis())
    }
}

/// Cubic-bezier motion easing curves spanning Material, Fluent, HIG, and Adwaita guidelines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MotionEasing {
    /// Balanced curve (`cubic-bezier(0.2, 0.0, 0.0, 1.0)`).
    #[default]
    Standard,
    /// Expressive decelerating curve (`cubic-bezier(0.05, 0.7, 0.1, 1.0)`).
    Emphasized,
    /// Enter curve (`cubic-bezier(0.0, 0.0, 0.0, 1.0)`).
    EmphasizedDecelerate,
    /// Exit curve (`cubic-bezier(0.3, 0.0, 0.8, 0.15)`).
    EmphasizedAccelerate,
    /// Windows Fluent fast-out slow-in (`cubic-bezier(0.1, 0.9, 0.2, 1.0)`).
    FluentFastOutSlowIn,
    /// Pure linear interpolation.
    Linear,
}

impl MotionEasing {
    /// Returns the normalized `(x1, y1, x2, y2)` cubic-bezier control points.
    #[must_use]
    pub const fn control_points(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Standard => (0.2, 0.0, 0.0, 1.0),
            Self::Emphasized => (0.05, 0.7, 0.1, 1.0),
            Self::EmphasizedDecelerate => (0.0, 0.0, 0.0, 1.0),
            Self::EmphasizedAccelerate => (0.3, 0.0, 0.8, 0.15),
            Self::FluentFastOutSlowIn => (0.1, 0.9, 0.2, 1.0),
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
        }
    }

    /// Evaluates the easing curve at normalized time `t` in `0.0..=1.0`.
    #[must_use]
    pub fn sample(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        if self == Self::Linear || t <= 0.0 || t >= 1.0 {
            return t;
        }
        let (x1, y1, x2, y2) = self.control_points();
        let u = solve_bezier_x(t, x1, x2);
        eval_bezier(u, y1, y2).clamp(0.0, 1.0)
    }
}

fn eval_bezier(u: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - u;
    3.0 * inv * inv * u * p1 + 3.0 * inv * u * u * p2 + u * u * u
}

fn eval_bezier_deriv(u: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - u;
    3.0 * inv * inv * p1 + 6.0 * inv * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
}

fn solve_bezier_x(target_x: f64, x1: f64, x2: f64) -> f64 {
    let mut u = target_x;
    for _ in 0..8 {
        let err = eval_bezier(u, x1, x2) - target_x;
        if err.abs() < 1e-7 {
            return u;
        }
        let d = eval_bezier_deriv(u, x1, x2);
        if d.abs() < 1e-6 {
            break;
        }
        u = (u - err / d).clamp(0.0, 1.0);
    }

    let mut lo = 0.0;
    let mut hi = 1.0;
    let mut mid = u;
    for _ in 0..24 {
        let x = eval_bezier(mid, x1, x2);
        if (x - target_x).abs() < 1e-7 {
            break;
        }
        if x < target_x {
            lo = mid;
        } else {
            hi = mid;
        }
        mid = 0.5 * (lo + hi);
    }
    mid
}

/// Underdamped / critically-damped / overdamped spring physics specification.
///
/// Used by Material 3 Expressive [`MotionScheme`] for spatial transitions (bounds, scale, morph,
/// offset) and non-overshooting effects transitions (opacity, color).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringSpec {
    /// Damping ratio (`< 1.0` produces spatial overshoot/bounce; `1.0` is critically damped).
    pub damping_ratio: f64,
    /// Spring stiffness constant (`k > 0`).
    pub stiffness: f64,
}

impl SpringSpec {
    /// Constructs a [`SpringSpec`] clamped to physically valid positive parameters.
    #[must_use]
    pub const fn new(damping_ratio: f64, stiffness: f64) -> Self {
        let zeta = if damping_ratio > 0.05 {
            damping_ratio
        } else {
            0.05
        };
        let k = if stiffness > 1.0 { stiffness } else { 1.0 };
        Self {
            damping_ratio: zeta,
            stiffness: k,
        }
    }

    /// M3 Expressive fast spatial spring (snappy bounce for small buttons/toggles).
    #[must_use]
    pub const fn expressive_fast_spatial() -> Self {
        Self::new(0.6, 800.0)
    }

    /// M3 Expressive default spatial spring (balanced overshoot for cards/panels).
    #[must_use]
    pub const fn expressive_default_spatial() -> Self {
        Self::new(0.8, 380.0)
    }

    /// M3 Expressive slow spatial spring (sheets/full-screen containers).
    #[must_use]
    pub const fn expressive_slow_spatial() -> Self {
        Self::new(0.8, 200.0)
    }

    /// M3 Expressive fast effects spring (no overshoot for hover/press tints & opacity).
    #[must_use]
    pub const fn expressive_fast_effects() -> Self {
        Self::new(1.0, 3800.0)
    }

    /// M3 Expressive default effects spring (critically damped, zero overshoot).
    #[must_use]
    pub const fn expressive_default_effects() -> Self {
        Self::new(1.0, 1600.0)
    }

    /// M3 Expressive slow effects spring (critically damped, zero overshoot).
    #[must_use]
    pub const fn expressive_slow_effects() -> Self {
        Self::new(1.0, 800.0)
    }

    /// Standard fast spatial spring.
    #[must_use]
    pub const fn standard_fast_spatial() -> Self {
        Self::new(0.9, 1400.0)
    }

    /// Standard default spatial spring.
    #[must_use]
    pub const fn standard_default_spatial() -> Self {
        Self::new(0.9, 700.0)
    }

    /// Standard slow spatial spring.
    #[must_use]
    pub const fn standard_slow_spatial() -> Self {
        Self::new(0.9, 300.0)
    }

    /// Standard fast effects spring.
    #[must_use]
    pub const fn standard_fast_effects() -> Self {
        Self::new(1.0, 3800.0)
    }

    /// Standard default effects spring.
    #[must_use]
    pub const fn standard_default_effects() -> Self {
        Self::new(1.0, 1600.0)
    }

    /// Standard slow effects spring.
    #[must_use]
    pub const fn standard_slow_effects() -> Self {
        Self::new(1.0, 800.0)
    }

    /// Analytical displacement `x(t)` starting at `0.0` with unit equilibrium target `1.0`.
    ///
    /// Underdamped springs (`damping_ratio < 1.0`) may slightly overshoot `1.0` cleanly before
    /// settling; critically damped / overdamped springs remain bounded in `0.0..=1.0`.
    #[must_use]
    pub fn sample(self, elapsed_secs: f64) -> f64 {
        if elapsed_secs <= 0.0 || !elapsed_secs.is_finite() {
            return 0.0;
        }
        let zeta = self.damping_ratio.max(0.05);
        let omega0 = self.stiffness.max(1.0).sqrt();
        let t = elapsed_secs;

        if (zeta - 1.0).abs() < 1e-4 {
            // Critically damped: 1 - e^(-w0*t) * (1 + w0*t)
            let decay = (-omega0 * t).exp();
            1.0 - decay * (1.0 + omega0 * t)
        } else if zeta < 1.0 {
            // Underdamped: 1 - e^(-zeta*w0*t) * (cos(wd*t) + (zeta/sqrt(1-zeta^2)) * sin(wd*t))
            let sq = (1.0 - zeta * zeta).sqrt();
            let wd = omega0 * sq;
            let decay = (-zeta * omega0 * t).exp();
            1.0 - decay * ((wd * t).cos() + (zeta / sq) * (wd * t).sin())
        } else {
            // Overdamped
            let gamma = omega0 * (zeta * zeta - 1.0).sqrt();
            let r1 = -zeta * omega0 + gamma;
            let r2 = -zeta * omega0 - gamma;
            let c1 = r2 / (r2 - r1);
            let c2 = -r1 / (r2 - r1);
            1.0 - c1 * (r1 * t).exp() - c2 * (r2 * t).exp()
        }
    }

    /// Returns `true` once the analytical decay envelope has dropped below `1e-3` (capped at `1.5s`).
    #[must_use]
    pub fn is_settled(self, elapsed_secs: f64) -> bool {
        if elapsed_secs >= 1.5 {
            return true;
        }
        if elapsed_secs <= 0.0 {
            return false;
        }
        let zeta = self.damping_ratio.max(0.05);
        let omega0 = self.stiffness.max(1.0).sqrt();
        let env_rate = if zeta <= 1.0 {
            zeta * omega0
        } else {
            omega0 * (zeta - (zeta * zeta - 1.0).sqrt())
        };
        (-env_rate * elapsed_secs).exp() * (1.0 + omega0 * elapsed_secs) < 1e-3
    }
}

/// Platform motion physics personality selector (`Standard` vs Material 3 `Expressive`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MotionScheme {
    /// Conservative critically- or near-critically-damped springs.
    #[default]
    Standard,
    /// Material 3 Expressive bouncier spatial springs paired with overshoot-free effects springs.
    Expressive,
}

impl MotionScheme {
    /// Fast spatial spring (`0.6/800` for Expressive, `0.9/1400` for Standard).
    #[must_use]
    pub const fn fast_spatial(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_fast_spatial(),
            Self::Expressive => SpringSpec::expressive_fast_spatial(),
        }
    }

    /// Default spatial spring (`0.8/380` for Expressive, `0.9/700` for Standard).
    #[must_use]
    pub const fn default_spatial(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_default_spatial(),
            Self::Expressive => SpringSpec::expressive_default_spatial(),
        }
    }

    /// Slow spatial spring (`0.8/200` for Expressive, `0.9/300` for Standard).
    #[must_use]
    pub const fn slow_spatial(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_slow_spatial(),
            Self::Expressive => SpringSpec::expressive_slow_spatial(),
        }
    }

    /// Fast effects spring (critically damped, zero overshoot).
    #[must_use]
    pub const fn fast_effects(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_fast_effects(),
            Self::Expressive => SpringSpec::expressive_fast_effects(),
        }
    }

    /// Default effects spring (critically damped, zero overshoot).
    #[must_use]
    pub const fn default_effects(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_default_effects(),
            Self::Expressive => SpringSpec::expressive_default_effects(),
        }
    }

    /// Slow effects spring (critically damped, zero overshoot).
    #[must_use]
    pub const fn slow_effects(self) -> SpringSpec {
        match self {
            Self::Standard => SpringSpec::standard_slow_effects(),
            Self::Expressive => SpringSpec::expressive_slow_effects(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_strictly_increasing() {
        let ordered = [
            MotionDuration::Short1,
            MotionDuration::Short2,
            MotionDuration::Short3,
            MotionDuration::Short4,
            MotionDuration::Medium1,
            MotionDuration::Medium2,
            MotionDuration::Medium3,
            MotionDuration::Medium4,
            MotionDuration::Long1,
            MotionDuration::Long2,
            MotionDuration::Long3,
            MotionDuration::Long4,
            MotionDuration::ExtraLong1,
            MotionDuration::ExtraLong2,
            MotionDuration::ExtraLong3,
            MotionDuration::ExtraLong4,
        ];
        for pair in ordered.windows(2) {
            assert!(pair[0].as_millis() < pair[1].as_millis());
        }
        assert_eq!(
            MotionDuration::Medium2.to_duration(),
            Duration::from_millis(300)
        );
    }

    #[test]
    fn easing_curves_are_bounded_and_monotonic() {
        for curve in [
            MotionEasing::Standard,
            MotionEasing::Emphasized,
            MotionEasing::EmphasizedDecelerate,
            MotionEasing::EmphasizedAccelerate,
            MotionEasing::FluentFastOutSlowIn,
            MotionEasing::Linear,
        ] {
            assert!((curve.sample(0.0) - 0.0).abs() < 1e-6);
            assert!((curve.sample(1.0) - 1.0).abs() < 1e-6);
            let mut prev = 0.0;
            for step in 1..=20 {
                let val = curve.sample(f64::from(step) / 20.0);
                assert!(
                    val >= prev - 1e-6,
                    "curve {curve:?} decreased at step {step}"
                );
                assert!(val <= 1.0 + 1e-6);
                prev = val;
            }
        }
    }

    #[test]
    fn spring_spec_spatial_overshoots_and_effects_are_monotonic() {
        for spatial in [
            MotionScheme::Expressive.fast_spatial(),
            MotionScheme::Expressive.default_spatial(),
        ] {
            assert_eq!(spatial.sample(0.0), 0.0);
            let mut max_val = 0.0_f64;
            for step in 1..=50 {
                let val = spatial.sample(f64::from(step) * 0.01);
                max_val = max_val.max(val);
            }
            assert!(
                max_val > 1.0,
                "expressive spatial spring {spatial:?} should overshoot 1.0 (peak={max_val})"
            );
            assert!(spatial.is_settled(1.5));
            assert!((spatial.sample(1.5) - 1.0).abs() < 1e-2);
        }

        for effects in [
            MotionScheme::Expressive.fast_effects(),
            MotionScheme::Expressive.default_effects(),
        ] {
            assert_eq!(effects.sample(0.0), 0.0);
            let mut prev = 0.0_f64;
            for step in 1..=50 {
                let val = effects.sample(f64::from(step) * 0.01);
                assert!(
                    val >= prev - 1e-6,
                    "effects spring {effects:?} oscillated backward at step {step}"
                );
                assert!(
                    val <= 1.0 + 1e-6,
                    "effects spring {effects:?} overshot 1.0: {val}"
                );
                prev = val;
            }
            assert!(effects.is_settled(1.5));
        }
    }
}
