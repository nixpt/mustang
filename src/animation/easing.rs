//! Easing functions for animations
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.

/// Easing function type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    /// Linear interpolation (no easing)
    Linear,
    /// Ease (default CSS ease)
    Ease,
    /// Ease-in (slow start)
    EaseIn,
    /// Ease-out (slow end)
    EaseOut,
    /// Ease-in-out (slow start and end)
    EaseInOut,
    /// Quadratic ease-in
    QuadIn,
    /// Quadratic ease-out
    QuadOut,
    /// Quadratic ease-in-out
    QuadInOut,
    /// Cubic ease-in
    CubicIn,
    /// Cubic ease-out
    CubicOut,
    /// Cubic ease-in-out
    CubicInOut,
    /// Quartic ease-in
    QuartIn,
    /// Quartic ease-out
    QuartOut,
    /// Quartic ease-in-out
    QuartInOut,
    /// Quintic ease-in
    QuintIn,
    /// Quintic ease-out
    QuintOut,
    /// Quintic ease-in-out
    QuintInOut,
    /// Sine ease-in
    SineIn,
    /// Sine ease-out
    SineOut,
    /// Sine ease-in-out
    SineInOut,
    /// Exponential ease-in
    ExpoIn,
    /// Exponential ease-out
    ExpoOut,
    /// Exponential ease-in-out
    ExpoInOut,
    /// Circular ease-in
    CircIn,
    /// Circular ease-out
    CircOut,
    /// Circular ease-in-out
    CircInOut,
    /// Elastic ease-in
    ElasticIn,
    /// Elastic ease-out
    ElasticOut,
    /// Elastic ease-in-out
    ElasticInOut,
    /// Back ease-in (overshoot start)
    BackIn,
    /// Back ease-out (overshoot end)
    BackOut,
    /// Back ease-in-out
    BackInOut,
    /// Bounce ease-in
    BounceIn,
    /// Bounce ease-out
    BounceOut,
    /// Bounce ease-in-out
    BounceInOut,
}

impl Default for EasingFunction {
    fn default() -> Self {
        EasingFunction::Ease
    }
}

impl EasingFunction {
    /// Apply the easing function to a progress value (0.0 - 1.0)
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            EasingFunction::Linear => linear(t),
            EasingFunction::Ease => ease(t),
            EasingFunction::EaseIn => ease_in(t),
            EasingFunction::EaseOut => ease_out(t),
            EasingFunction::EaseInOut => ease_in_out(t),
            EasingFunction::QuadIn => quad_in(t),
            EasingFunction::QuadOut => quad_out(t),
            EasingFunction::QuadInOut => quad_in_out(t),
            EasingFunction::CubicIn => cubic_in(t),
            EasingFunction::CubicOut => cubic_out(t),
            EasingFunction::CubicInOut => cubic_in_out(t),
            EasingFunction::QuartIn => quart_in(t),
            EasingFunction::QuartOut => quart_out(t),
            EasingFunction::QuartInOut => quart_in_out(t),
            EasingFunction::QuintIn => quint_in(t),
            EasingFunction::QuintOut => quint_out(t),
            EasingFunction::QuintInOut => quint_in_out(t),
            EasingFunction::SineIn => sine_in(t),
            EasingFunction::SineOut => sine_out(t),
            EasingFunction::SineInOut => sine_in_out(t),
            EasingFunction::ExpoIn => expo_in(t),
            EasingFunction::ExpoOut => expo_out(t),
            EasingFunction::ExpoInOut => expo_in_out(t),
            EasingFunction::CircIn => circ_in(t),
            EasingFunction::CircOut => circ_out(t),
            EasingFunction::CircInOut => circ_in_out(t),
            EasingFunction::ElasticIn => elastic_in(t),
            EasingFunction::ElasticOut => elastic_out(t),
            EasingFunction::ElasticInOut => elastic_in_out(t),
            EasingFunction::BackIn => back_in(t),
            EasingFunction::BackOut => back_out(t),
            EasingFunction::BackInOut => back_in_out(t),
            EasingFunction::BounceIn => bounce_in(t),
            EasingFunction::BounceOut => bounce_out(t),
            EasingFunction::BounceInOut => bounce_in_out(t),
        }
    }

    /// Parse easing function from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "linear" => Some(EasingFunction::Linear),
            "ease" => Some(EasingFunction::Ease),
            "ease-in" | "easein" => Some(EasingFunction::EaseIn),
            "ease-out" | "easeout" => Some(EasingFunction::EaseOut),
            "ease-in-out" | "easeinout" => Some(EasingFunction::EaseInOut),
            "quad-in" | "quadin" => Some(EasingFunction::QuadIn),
            "quad-out" | "quadout" => Some(EasingFunction::QuadOut),
            "quad-in-out" | "quadinout" => Some(EasingFunction::QuadInOut),
            "cubic-in" | "cubicin" => Some(EasingFunction::CubicIn),
            "cubic-out" | "cubicout" => Some(EasingFunction::CubicOut),
            "cubic-in-out" | "cubicinout" => Some(EasingFunction::CubicInOut),
            "quart-in" | "quartin" => Some(EasingFunction::QuartIn),
            "quart-out" | "quartout" => Some(EasingFunction::QuartOut),
            "quart-in-out" | "quartinout" => Some(EasingFunction::QuartInOut),
            "quint-in" | "quintin" => Some(EasingFunction::QuintIn),
            "quint-out" | "quintout" => Some(EasingFunction::QuintOut),
            "quint-in-out" | "quintinout" => Some(EasingFunction::QuintInOut),
            "sine-in" | "sinein" => Some(EasingFunction::SineIn),
            "sine-out" | "sineout" => Some(EasingFunction::SineOut),
            "sine-in-out" | "sineinout" => Some(EasingFunction::SineInOut),
            "expo-in" | "expoin" => Some(EasingFunction::ExpoIn),
            "expo-out" | "expoout" => Some(EasingFunction::ExpoOut),
            "expo-in-out" | "expoinout" => Some(EasingFunction::ExpoInOut),
            "circ-in" | "circin" => Some(EasingFunction::CircIn),
            "circ-out" | "circout" => Some(EasingFunction::CircOut),
            "circ-in-out" | "circinout" => Some(EasingFunction::CircInOut),
            "elastic-in" | "elasticin" => Some(EasingFunction::ElasticIn),
            "elastic-out" | "elasticout" => Some(EasingFunction::ElasticOut),
            "elastic-in-out" | "elasticinout" => Some(EasingFunction::ElasticInOut),
            "back-in" | "backin" => Some(EasingFunction::BackIn),
            "back-out" | "backout" => Some(EasingFunction::BackOut),
            "back-in-out" | "backinout" => Some(EasingFunction::BackInOut),
            "bounce-in" | "bouncein" => Some(EasingFunction::BounceIn),
            "bounce-out" | "bounceout" => Some(EasingFunction::BounceOut),
            "bounce-in-out" | "bounceinout" => Some(EasingFunction::BounceInOut),
            _ => None,
        }
    }
}

// Linear
fn linear(t: f32) -> f32 {
    t
}

// CSS default ease (cubic-bezier(0.25, 0.1, 0.25, 1.0))
fn ease(t: f32) -> f32 {
    cubic_bezier(t, 0.25, 0.1, 0.25, 1.0)
}

// CSS ease-in (cubic-bezier(0.42, 0, 1.0, 1.0))
fn ease_in(t: f32) -> f32 {
    cubic_bezier(t, 0.42, 0.0, 1.0, 1.0)
}

// CSS ease-out (cubic-bezier(0, 0, 0.58, 1.0))
fn ease_out(t: f32) -> f32 {
    cubic_bezier(t, 0.0, 0.0, 0.58, 1.0)
}

// CSS ease-in-out (cubic-bezier(0.42, 0, 0.58, 1.0))
fn ease_in_out(t: f32) -> f32 {
    cubic_bezier(t, 0.42, 0.0, 0.58, 1.0)
}

fn cubic_bezier(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    let mut t = x;
    for _ in 0..8 {
        let one_minus_t = 1.0 - t;
        let bezier_x = 3.0 * one_minus_t * one_minus_t * t * x1
            + 3.0 * one_minus_t * t * t * x2
            + t * t * t;
        let bezier_x_d = 3.0 * one_minus_t * one_minus_t * x1
            + 6.0 * one_minus_t * t * (x2 - x1)
            + 3.0 * t * t * (1.0 - x2);
        let dx = bezier_x - x;
        if dx.abs() < 1e-6 {
            break;
        }
        if bezier_x_d.abs() < 1e-6 {
            t = (t + x) * 0.5;
            continue;
        }
        t = (t - dx / bezier_x_d).clamp(0.0, 1.0);
    }

    let one_minus_t = 1.0 - t;
    3.0 * one_minus_t * one_minus_t * t * y1
        + 3.0 * one_minus_t * t * t * y2
        + t * t * t
}

// Quadratic
fn quad_in(t: f32) -> f32 {
    t * t
}

fn quad_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

fn quad_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

// Cubic
fn cubic_in(t: f32) -> f32 {
    t * t * t
}

fn cubic_out(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

fn cubic_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

// Quartic
fn quart_in(t: f32) -> f32 {
    t * t * t * t
}

fn quart_out(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u * u
}

fn quart_in_out(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
    }
}

// Quintic
fn quint_in(t: f32) -> f32 {
    t * t * t * t * t
}

fn quint_out(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u * u * u
}

fn quint_in_out(t: f32) -> f32 {
    if t < 0.5 {
        16.0 * t * t * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
    }
}

// Sine
fn sine_in(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::PI / 2.0).cos()
}

fn sine_out(t: f32) -> f32 {
    (t * std::f32::consts::PI / 2.0).sin()
}

fn sine_in_out(t: f32) -> f32 {
    -(std::f32::consts::PI * t).cos() / 2.0 + 0.5
}

// Exponential
fn expo_in(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else {
        2.0_f32.powf(10.0 * (t - 1.0))
    }
}

fn expo_out(t: f32) -> f32 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

fn expo_in_out(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else if t < 0.5 {
        2.0_f32.powf(20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
    }
}

// Circular
fn circ_in(t: f32) -> f32 {
    1.0 - (1.0 - t * t).sqrt()
}

fn circ_out(t: f32) -> f32 {
    (1.0 - (t - 1.0) * (t - 1.0)).sqrt()
}

fn circ_in_out(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - (1.0 - 4.0 * t * t).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0 * t + 2.0) * (-2.0 * t + 2.0)).sqrt() + 1.0) / 2.0
    }
}

// Elastic
fn elastic_in(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        -2.0_f32.powf(10.0 * t - 10.0)
            * ((t * 10.0 - 10.75) * (2.0 * std::f32::consts::PI) / 3.0).sin()
    }
}

fn elastic_out(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * (2.0 * std::f32::consts::PI) / 3.0).sin()
            + 1.0
    }
}

fn elastic_in_out(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else if t < 0.5 {
        -(2.0_f32.powf(20.0 * t - 10.0)
            * ((20.0 * t - 11.125) * (2.0 * std::f32::consts::PI) / 4.5).sin())
            / 2.0
    } else {
        (2.0_f32.powf(-20.0 * t + 10.0)
            * ((20.0 * t - 11.125) * (2.0 * std::f32::consts::PI) / 4.5).sin())
            / 2.0
            + 1.0
    }
}

// Back
const BACK_CONSTANT: f32 = 1.70158;

fn back_in(t: f32) -> f32 {
    let c = BACK_CONSTANT;
    t * t * ((c + 1.0) * t - c)
}

fn back_out(t: f32) -> f32 {
    let c = BACK_CONSTANT;
    let u = t - 1.0;
    u * u * ((c + 1.0) * u + c) + 1.0
}

fn back_in_out(t: f32) -> f32 {
    let c = BACK_CONSTANT * 1.525;
    if t < 0.5 {
        (2.0 * t).powi(2) * ((c + 1.0) * 2.0 * t - c) / 2.0
    } else {
        ((2.0 * t - 2.0).powi(2) * ((c + 1.0) * (2.0 * t - 2.0) + c) + 2.0) / 2.0
    }
}

// Bounce
fn bounce_out(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

fn bounce_in(t: f32) -> f32 {
    1.0 - bounce_out(1.0 - t)
}

fn bounce_in_out(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert_eq!(EasingFunction::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert_eq!(EasingFunction::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_ease_bounds() {
        let easings = vec![
            EasingFunction::Ease,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
            EasingFunction::QuadIn,
            EasingFunction::QuadOut,
            EasingFunction::CubicIn,
            EasingFunction::CubicOut,
            EasingFunction::SineIn,
            EasingFunction::SineOut,
            EasingFunction::ExpoIn,
            EasingFunction::ExpoOut,
        ];

        for easing in easings {
            let result = easing.apply(0.0);
            assert!(
                (result - 0.0).abs() < 0.001,
                "{:?} at 0.0 should be ~0",
                easing
            );

            let result = easing.apply(1.0);
            assert!(
                (result - 1.0).abs() < 0.001,
                "{:?} at 1.0 should be ~1",
                easing
            );
        }
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            EasingFunction::from_str("linear"),
            Some(EasingFunction::Linear)
        );
        assert_eq!(EasingFunction::from_str("ease"), Some(EasingFunction::Ease));
        assert_eq!(
            EasingFunction::from_str("ease-in"),
            Some(EasingFunction::EaseIn)
        );
        assert_eq!(
            EasingFunction::from_str("easeOut"),
            Some(EasingFunction::EaseOut)
        );
        assert_eq!(
            EasingFunction::from_str("cubic-in"),
            Some(EasingFunction::CubicIn)
        );
        assert_eq!(
            EasingFunction::from_str("bounce-out"),
            Some(EasingFunction::BounceOut)
        );
        assert_eq!(EasingFunction::from_str("unknown"), None);
    }

    #[test]
    fn test_quad_in() {
        assert_eq!(EasingFunction::QuadIn.apply(0.0), 0.0);
        assert_eq!(EasingFunction::QuadIn.apply(0.5), 0.25);
        assert_eq!(EasingFunction::QuadIn.apply(1.0), 1.0);
    }

    #[test]
    fn test_bounce() {
        // Bounce should start at 0 and end at 1
        assert_eq!(EasingFunction::BounceOut.apply(0.0), 0.0);
        assert!((EasingFunction::BounceOut.apply(1.0) - 1.0).abs() < 0.001);
    }
}
