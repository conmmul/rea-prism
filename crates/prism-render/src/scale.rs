//! Mapping data values onto the plotting area, and choosing tick positions.

use prism_doc::graph::{Axis, Scale};

/// A configured mapping from data values to points along one screen axis.
#[derive(Debug, Clone)]
pub struct AxisScale {
    pub scale: Scale,
    /// Data-space bounds. For a log scale these are the logs of the limits.
    pub min: f64,
    pub max: f64,
    /// Screen position of `min` and of `max`, in points.
    pub screen_min: f32,
    pub screen_max: f32,
    pub reversed: bool,
    pub categories: usize,
}

impl AxisScale {
    /// Transform a data value into the space the bounds are expressed in.
    fn forward(&self, v: f64) -> f64 {
        match self.scale {
            Scale::Log(base) => {
                if v > 0.0 {
                    v.log(base)
                } else {
                    f64::NEG_INFINITY
                }
            }
            _ => v,
        }
    }

    fn inverse(&self, v: f64) -> f64 {
        match self.scale {
            Scale::Log(base) => base.powf(v),
            _ => v,
        }
    }

    /// Map a data value to a screen coordinate.
    pub fn to_screen(&self, v: f64) -> f32 {
        let t = if (self.max - self.min).abs() < f64::EPSILON {
            0.5
        } else {
            (self.forward(v) - self.min) / (self.max - self.min)
        };
        let t = if self.reversed { 1.0 - t } else { t };
        self.screen_min + (self.screen_max - self.screen_min) * t as f32
    }

    /// Map a screen coordinate back to a data value.
    pub fn to_data(&self, s: f32) -> f64 {
        let span = self.screen_max - self.screen_min;
        let mut t = if span.abs() < f32::EPSILON { 0.5 } else { (s - self.screen_min) / span };
        if self.reversed {
            t = 1.0 - t;
        }
        self.inverse(self.min + (self.max - self.min) * t as f64)
    }

    /// Screen position of the centre of categorical slot `i`. A categorical axis
    /// runs from 0 to `categories`, so slot `i` spans `i..i+1`.
    pub fn category_center(&self, i: usize) -> f32 {
        self.to_screen(i as f64 + 0.5)
    }

    /// Width in points of one categorical slot.
    pub fn category_width(&self) -> f32 {
        let n = self.categories.max(1) as f32;
        (self.screen_max - self.screen_min).abs() / n
    }

    pub fn is_valid(&self, v: f64) -> bool {
        let f = self.forward(v);
        f.is_finite()
    }
}

/// A tick to draw, with the label Prism would write beside it.
#[derive(Debug, Clone)]
pub struct Tick {
    pub value: f64,
    pub label: Option<String>,
    pub major: bool,
}

/// Pick limits that cover `min..=max` and end on round numbers, the way Prism
/// does when an axis is left on automatic.
pub fn nice_bounds(min: f64, max: f64, scale: Scale) -> (f64, f64) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if let Scale::Log(base) = scale {
        let lo = if min > 0.0 { min.log(base).floor() } else { 0.0 };
        let hi = if max > 0.0 { max.log(base).ceil() } else { lo + 1.0 };
        return (base.powf(lo), base.powf(if hi > lo { hi } else { lo + 1.0 }));
    }
    if (max - min).abs() < f64::EPSILON {
        let pad = if min.abs() < f64::EPSILON { 1.0 } else { min.abs() * 0.1 };
        return (min - pad, max + pad);
    }
    let step = nice_step((max - min) / 5.0);
    let lo = (min / step).floor() * step;
    let hi = (max / step).ceil() * step;
    (lo, hi)
}

/// Round a raw interval up to 1, 2, 2.5 or 5 times a power of ten.
pub fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 || !raw.is_finite() {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 2.5 {
        2.5
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * mag
}

/// Generate the ticks for an axis, honouring any custom ticks it carries.
pub fn ticks(axis: &Axis, s: &AxisScale) -> Vec<Tick> {
    if !axis.custom_ticks.is_empty() {
        return axis
            .custom_ticks
            .iter()
            .map(|t| Tick {
                value: t.value,
                label: Some(t.label.clone().unwrap_or_else(|| format_value(t.value, axis))),
                major: true,
            })
            .collect();
    }

    match axis.scale {
        Scale::Categorical => (0..axis.categories.len())
            .map(|i| Tick {
                value: i as f64 + 0.5,
                label: axis.categories.get(i).cloned(),
                major: true,
            })
            .collect(),
        Scale::Log(base) => {
            let mut out = Vec::new();
            let lo = s.min.floor() as i32;
            let hi = s.max.ceil() as i32;
            // Guard against an absurd number of decades.
            if hi - lo > 40 {
                return out;
            }
            for d in lo..=hi {
                let v = base.powi(d);
                if v < s.inverse(s.min) * 0.999 || v > s.inverse(s.max) * 1.001 {
                    continue;
                }
                out.push(Tick { value: v, label: Some(format_value(v, axis)), major: true });
                if axis.minor_ticks > 0 {
                    for m in 2..(base as i32).max(2) {
                        let mv = v * m as f64;
                        if mv <= s.inverse(s.max) {
                            out.push(Tick { value: mv, label: None, major: false });
                        }
                    }
                }
            }
            out
        }
        Scale::Linear => {
            let step = axis.major_interval.unwrap_or_else(|| nice_step((s.max - s.min) / 5.0));
            if step <= 0.0 || !step.is_finite() {
                return Vec::new();
            }
            let count = ((s.max - s.min) / step).abs();
            if count > 200.0 {
                return Vec::new();
            }
            let mut out = Vec::new();
            let first = (s.min / step).ceil();
            let mut k = first;
            while k * step <= s.max + step * 1e-9 {
                let v = k * step;
                out.push(Tick { value: v, label: Some(format_value(v, axis)), major: true });
                if axis.minor_ticks > 0 {
                    let sub = step / (axis.minor_ticks + 1) as f64;
                    for m in 1..=axis.minor_ticks {
                        let mv = v + sub * m as f64;
                        if mv < s.max {
                            out.push(Tick { value: mv, label: None, major: false });
                        }
                    }
                }
                k += 1.0;
            }
            out
        }
    }
}

/// Render a tick value the way the axis asks for.
pub fn format_value(v: f64, axis: &Axis) -> String {
    use prism_doc::graph::NumberFormat;
    match axis.number_format {
        NumberFormat::Scientific => format!("{v:e}"),
        NumberFormat::Percent => {
            let d = axis.decimals.unwrap_or(0) as usize;
            format!("{:.*}%", d, v * 100.0)
        }
        NumberFormat::Power10 => {
            if v > 0.0 {
                let e = v.log10();
                if (e - e.round()).abs() < 1e-9 {
                    return format!("10^{}", e.round() as i64);
                }
            }
            format_plain(v, axis.decimals)
        }
        NumberFormat::Antilog => format_plain(10f64.powf(v), axis.decimals),
        NumberFormat::Decimal => format_plain(v, axis.decimals),
    }
}

fn format_plain(v: f64, decimals: Option<u32>) -> String {
    match decimals {
        Some(d) => format!("{:.*}", d as usize, v),
        None => {
            // Trim a value that is integral in all but floating-point noise.
            if (v - v.round()).abs() < 1e-9 * v.abs().max(1.0) {
                format!("{}", v.round() as i64)
            } else {
                let s = format!("{v:.6}");
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(min: f64, max: f64) -> AxisScale {
        AxisScale {
            scale: Scale::Linear,
            min,
            max,
            screen_min: 0.0,
            screen_max: 100.0,
            reversed: false,
            categories: 0,
        }
    }

    #[test]
    fn linear_mapping_is_proportional() {
        let s = linear(0.0, 10.0);
        assert_eq!(s.to_screen(0.0), 0.0);
        assert_eq!(s.to_screen(10.0), 100.0);
        assert_eq!(s.to_screen(5.0), 50.0);
        assert!((s.to_data(50.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn log_mapping_places_decades_evenly() {
        let s = AxisScale { scale: Scale::Log(10.0), min: 0.0, max: 3.0, ..linear(0.0, 3.0) };
        assert!((s.to_screen(1.0) - 0.0).abs() < 1e-5);
        assert!((s.to_screen(1000.0) - 100.0).abs() < 1e-5);
        assert!((s.to_screen(10.0) - 33.333).abs() < 0.01);
    }

    #[test]
    fn nice_steps_land_on_round_numbers() {
        assert_eq!(nice_step(0.3), 0.5);
        assert_eq!(nice_step(7.0), 10.0);
        assert_eq!(nice_step(1.1), 2.0);
    }

    #[test]
    fn automatic_bounds_cover_the_data() {
        let (lo, hi) = nice_bounds(3.0, 47.0, Scale::Linear);
        assert!(lo <= 3.0 && hi >= 47.0);
        let (lo, hi) = nice_bounds(2.0, 800.0, Scale::Log(10.0));
        assert_eq!((lo, hi), (1.0, 1000.0));
    }

    #[test]
    fn reversed_axes_flip_the_mapping() {
        let mut s = linear(0.0, 10.0);
        s.reversed = true;
        assert_eq!(s.to_screen(0.0), 100.0);
        assert_eq!(s.to_screen(10.0), 0.0);
    }
}
