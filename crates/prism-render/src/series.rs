//! Turning a Prism data table into the values a plot draws.
//!
//! A Y dataset occupies one or more physical subcolumns depending on the table's
//! subcolumn format: replicates side by side, or a mean with its spread. This
//! module reduces either shape to the points and error bars a plot needs, applying
//! the summary the graph asks for.

use prism_doc::graph::{Center, Spread};
use prism_doc::model::{DataTable, SubcolumnFormat};

/// One plotted observation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    /// Distance from `y` to the lower whisker end, when there is one.
    pub err_low: Option<f64>,
    /// Distance from `y` to the upper whisker end.
    pub err_high: Option<f64>,
    /// Row this came from, for hit-testing back to the sheet.
    pub row: usize,
}

impl DataPoint {
    fn plain(x: f64, y: f64, row: usize) -> Self {
        DataPoint { x, y, err_low: None, err_high: None, row }
    }
}

/// Summary statistics over a set of replicates.
#[derive(Debug, Clone, Copy, Default)]
pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
}

impl Summary {
    pub fn sem(&self) -> f64 {
        if self.n > 1 {
            self.sd / (self.n as f64).sqrt()
        } else {
            0.0
        }
    }

    /// Half-width of a normal-approximation confidence interval at `level`
    /// (0.95 for the usual 95% interval).
    pub fn ci_half_width(&self, level: f64) -> f64 {
        // A normal quantile is adequate for drawing; it is not a statistical result.
        let z = match (level * 100.0).round() as i32 {
            90 => 1.645,
            99 => 2.576,
            _ => 1.960,
        };
        z * self.sem()
    }

    pub fn iqr(&self) -> f64 {
        self.q3 - self.q1
    }
}

/// Mean, spread and order statistics of a sample.
pub fn summarize(values: &[f64]) -> Summary {
    if values.is_empty() {
        return Summary::default();
    }
    let n = values.len();
    let mean = values.iter().sum::<f64>() / n as f64;
    let var = if n > 1 {
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
    } else {
        0.0
    };
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Summary {
        n,
        mean,
        sd: var.sqrt(),
        min: sorted[0],
        max: sorted[n - 1],
        median: quantile(&sorted, 0.5),
        q1: quantile(&sorted, 0.25),
        q3: quantile(&sorted, 0.75),
    }
}

/// Linear-interpolation quantile of an already sorted sample.
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
    }
}

/// Every replicate of one dataset in one row.
pub fn replicates(table: &DataTable, dataset: usize, row: usize) -> Vec<f64> {
    let base = table.y_offset(dataset);
    match table.data_format {
        SubcolumnFormat::Single => {
            table.values.get(row, base).as_number().into_iter().collect()
        }
        SubcolumnFormat::Replicates => (0..table.y_width())
            .filter_map(|k| table.values.get(row, base + k).as_number())
            .collect(),
        // Summarised formats store a mean, not the underlying replicates.
        _ => table.values.get(row, base).as_number().into_iter().collect(),
    }
}

/// Every value of one dataset across all rows, used by column-style plots.
pub fn column_values(table: &DataTable, dataset: usize) -> Vec<f64> {
    (0..table.values.rows())
        .flat_map(|r| replicates(table, dataset, r))
        .collect()
}

/// The error half-widths already stored in the table, if its format carries them.
fn stored_error(table: &DataTable, dataset: usize, row: usize) -> Option<(f64, f64)> {
    let base = table.y_offset(dataset);
    let get = |k: usize| table.values.get(row, base + k).as_number();
    match table.data_format {
        SubcolumnFormat::MeanSd | SubcolumnFormat::MeanSem | SubcolumnFormat::MeanCv => {
            let e = get(1)?;
            Some((e, e))
        }
        SubcolumnFormat::MeanSdN | SubcolumnFormat::MeanSemN | SubcolumnFormat::MeanCvN => {
            let e = get(1)?;
            Some((e, e))
        }
        SubcolumnFormat::LowHigh => {
            let mean = get(0)?;
            let lo = get(0)?;
            let hi = get(1)?;
            let _ = mean;
            Some((0.0, hi - lo))
        }
        SubcolumnFormat::UpperLowerLimits => {
            let mean = get(0)?;
            let lower = get(1)?;
            let upper = get(2)?;
            Some((mean - lower, upper - mean))
        }
        _ => None,
    }
}

/// Points for an XY-style plot: one mark per row, summarising replicates.
pub fn xy_points(
    table: &DataTable,
    dataset: usize,
    center: Center,
    spread: Spread,
) -> Vec<DataPoint> {
    let mut out = Vec::new();
    for row in 0..table.values.rows() {
        let Some(x) = table.x_at(row) else { continue };
        let reps = replicates(table, dataset, row);
        if reps.is_empty() {
            continue;
        }

        if center == Center::None && table.data_format == SubcolumnFormat::Replicates {
            // Draw every replicate at the same X.
            out.extend(reps.iter().map(|&y| DataPoint::plain(x, y, row)));
            continue;
        }

        let s = summarize(&reps);
        let y = match center {
            Center::Median => s.median,
            Center::GeometricMean => geometric_mean(&reps),
            _ => s.mean,
        };
        let (lo, hi) = error_for(table, dataset, row, &s, spread);
        out.push(DataPoint { x, y, err_low: lo, err_high: hi, row });
    }
    out
}

fn error_for(
    table: &DataTable,
    dataset: usize,
    row: usize,
    s: &Summary,
    spread: Spread,
) -> (Option<f64>, Option<f64>) {
    match spread {
        Spread::None => (None, None),
        // "From table" means whatever spread the table's own shape implies: the
        // stored error subcolumns when it has them, and the SD of the replicates
        // when it stores those instead. This is what Prism shows by default.
        Spread::FromTable => match stored_error(table, dataset, row) {
            Some((l, h)) => (Some(l), Some(h)),
            None if s.n > 1 => (Some(s.sd), Some(s.sd)),
            None => (None, None),
        },
        Spread::Sd if s.n > 1 => (Some(s.sd), Some(s.sd)),
        Spread::Sem if s.n > 1 => (Some(s.sem()), Some(s.sem())),
        Spread::Ci if s.n > 1 => {
            let h = s.ci_half_width(0.95);
            (Some(h), Some(h))
        }
        Spread::Range if s.n > 1 => (Some(s.mean - s.min), Some(s.max - s.mean)),
        // A single value, or a table whose format already holds the spread.
        _ => match stored_error(table, dataset, row) {
            Some((l, h)) => (Some(l), Some(h)),
            None => (None, None),
        },
    }
}

fn geometric_mean(values: &[f64]) -> f64 {
    let positive: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
    if positive.is_empty() {
        return f64::NAN;
    }
    let s: f64 = positive.iter().map(|v| v.ln()).sum();
    (s / positive.len() as f64).exp()
}

/// The data-space extent of everything a set of series will draw, including the
/// reach of their error bars.
pub fn extent(points: &[Vec<DataPoint>]) -> Option<(f64, f64, f64, f64)> {
    let mut x0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y0 = f64::INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for series in points {
        for p in series {
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y - p.err_low.unwrap_or(0.0));
            y1 = y1.max(p.y + p.err_high.unwrap_or(0.0));
        }
    }
    (x0.is_finite() && y0.is_finite()).then_some((x0, x1, y0, y1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_matches_hand_computed_values() {
        let s = summarize(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(s.n, 8);
        assert!((s.mean - 5.0).abs() < 1e-12);
        // Sample standard deviation, the one Prism reports.
        assert!((s.sd - 2.138089935299395).abs() < 1e-12);
        assert_eq!(s.min, 2.0);
        assert_eq!(s.max, 9.0);
        assert!((s.median - 4.5).abs() < 1e-12);
    }

    #[test]
    fn sem_shrinks_with_sample_size() {
        let s = summarize(&[1.0, 2.0, 3.0, 4.0]);
        assert!((s.sem() - s.sd / 2.0).abs() < 1e-12);
    }

    #[test]
    fn quantiles_interpolate() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert!((quantile(&v, 0.5) - 2.5).abs() < 1e-12);
        assert!((quantile(&v, 0.25) - 1.75).abs() < 1e-12);
    }
}
