//! Kernel density estimation: the estimator behind Plot's `density` and
//! `violin` marks.
//!
//! Plot uses d3's `density` with an Epanechnikov kernel by default. This
//! module reproduces that: a Gaussian kernel scaled to the sample's spread,
//! evaluated over a grid.

use crate::value::Value;

/// The kernel a density estimate smooths its samples with, Plot's `kde`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kernel {
    /// The default Gaussian kernel, `exp(-½ x²)`.
    #[default]
    Gaussian,
    /// Epanechnikov: `¾ (1 - x²)` inside the unit interval.
    Epanechnikov,
    /// A triangular kernel.
    Triangular,
    /// A uniform kernel.
    Uniform,
}

impl Kernel {
    fn weight(self, x: f64) -> f64 {
        match self {
            // The standard normal density, which d3's `density` divides by
            // the bandwidth after evaluating.
            Kernel::Gaussian => (-0.5 * x * x).exp() / (std::f64::consts::TAU).sqrt(),
            Kernel::Epanechnikov => {
                if x.abs() <= 1. {
                    0.75 * (1. - x * x)
                } else {
                    0.
                }
            }
            Kernel::Triangular => {
                if x.abs() <= 1. {
                    1. - x.abs()
                } else {
                    0.
                }
            }
            Kernel::Uniform => {
                if x.abs() <= 1. {
                    0.5
                } else {
                    0.
                }
            }
        }
    }
}

/// Estimate a density over `values`, returning the density and the domain it
/// spans.
///
/// `steps` is the number of grid points; Plot's default is about 100. The
/// bandwidth is the sample's standard deviation scaled by the rule of thumb
/// `1.06 · n^(-1/5)`, which is what d3 uses.
pub fn density_of(values: &[Value], steps: usize) -> Option<(Vec<f64>, [f64; 2])> {
    let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
    if numbers.is_empty() {
        return None;
    }

    let extent = crate::scale::extent(numbers.iter().map(|n| Value::Number(*n)))?;
    let [lo, hi] = extent;
    let (lo, hi) = if lo == hi {
        (lo - 0.5, hi + 0.5)
    } else {
        (lo, hi)
    };
    let steps = steps.max(8);

    let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
    let variance = numbers.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / numbers.len() as f64;
    let deviation = variance.sqrt();
    let bandwidth = if deviation == 0. {
        // A constant sample still needs a width, or every point divides by
        // zero.
        (hi - lo) / steps as f64
    } else {
        deviation * 1.06 * (numbers.len() as f64).powf(-0.2)
    }
    .max(f64::MIN_POSITIVE);

    let kernel = Kernel::default();
    let mut density = Vec::with_capacity(steps);
    for index in 0..steps {
        let position = lo + (hi - lo) * index as f64 / (steps - 1) as f64;
        let estimate: f64 = numbers
            .iter()
            .map(|sample| kernel.weight((position - sample) / bandwidth))
            .sum();
        density.push(estimate / (numbers.len() as f64 * bandwidth));
    }

    Some((density, [lo, hi]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_is_centered_on_the_data() {
        let values: Vec<Value> = (0..100).map(|i| Value::Number(i as f64)).collect();
        let (density, domain) = density_of(&values, 400).unwrap();
        let step = (domain[1] - domain[0]) / (density.len() - 1) as f64;

        // The grid spans the data only, so the Gaussian's tails are cut off and
        // the integral falls short of one; what must hold is that the estimate
        // is centered where the data is.
        let area: f64 = density.windows(2).map(|w| (w[0] + w[1]) / 2. * step).sum();
        assert!(area > 0.8 && area <= 1.0, "area was {area}");

        let mass: f64 = density.iter().sum::<f64>() * step;
        let mean = density
            .iter()
            .enumerate()
            .map(|(index, value)| (domain[0] + step * index as f64) * value)
            .sum::<f64>()
            * step
            / mass;
        assert!((mean - 49.5).abs() < 1.5, "mean was {mean}");
    }

    #[test]
    fn density_peaks_around_the_mode() {
        // Two clusters, the larger one on the right.
        let mut values: Vec<Value> = (0..20).map(|_| Value::Number(0.)).collect();
        values.extend((0..60).map(|_| Value::Number(10.)));
        let (density, _) = density_of(&values, 50).unwrap();
        let peak = density
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(peak > density.len() / 2);
    }

    #[test]
    fn constant_samples_do_not_divide_by_zero() {
        let values = vec![Value::Number(5.); 10];
        let (density, _) = density_of(&values, 20).unwrap();
        assert!(density.iter().all(|d| d.is_finite()));
    }

    #[test]
    fn kernels_peak_at_zero() {
        for kernel in [Kernel::Gaussian, Kernel::Epanechnikov, Kernel::Triangular] {
            assert!(kernel.weight(0.) > kernel.weight(0.5));
        }
        // A uniform kernel is flat inside its support, which is the point.
        assert_eq!(Kernel::Uniform.weight(0.), Kernel::Uniform.weight(0.5));
    }
}
