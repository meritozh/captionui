//! The statistical marks: a least-squares fit with its confidence band, and a
//! Bollinger band.
//!
//! Both summarize a series rather than draw its data points, so they group the
//! rows themselves and paint a line and a band. The fit runs in pixels, which
//! is what Plot's `linearRegression` does: its channels are already projected
//! by the scales when the mark renders, so the line is the least-squares fit
//! *as drawn*.

use gpui::{Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::render::basic::{DrawContext, series_groups};
use crate::value::Value;

/// Draw `linearRegressionY` (`along_x`) or `linearRegressionX`.
pub fn draw_linear_regression(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, along_x: bool) {
    let style = &prepared.mark.style;
    let stroke_width = style.stroke_width;
    let opacity = style.fill_opacity;

    ctx.clipped(|ctx| {
        for group in series_groups(prepared) {
            // The fit is computed on the drawn positions: the independent
            // coordinate is the one the line runs along.
            let points: Vec<(f64, f64)> = group
                .iter()
                .filter_map(|(_, row)| {
                    let placed = crate::render::basic::position(prepared, ctx, row)?;
                    Some(if along_x {
                        (placed.y.as_f32() as f64, placed.x.as_f32() as f64)
                    } else {
                        (placed.x.as_f32() as f64, placed.y.as_f32() as f64)
                    })
                })
                .collect();
            if points.len() < 2 {
                continue;
            }
            let Some(fit) = Fit::of(&points) else {
                continue;
            };
            // The band takes the mark's fill and the line its stroke, as
            // Plot's `linearRegression` does.
            let colors = crate::render::basic::Colors::resolve(prepared, ctx);
            let line_color = group
                .first()
                .map(|(index, _)| colors.stroke(*index))
                .unwrap_or_else(|| ctx.default_stroke(prepared));
            let (low, high) = extent(&points);
            let band = (style.ci > 0. && style.ci < 1. && points.len() > 2)
                .then(|| fit.confidence_band(&points, style.ci, 4.))
                .flatten();

            if let Some(band) = band {
                let polygon: Vec<Point<Pixels>> = band_polygon(&band, along_x)
                    .into_iter()
                    .map(|point| ctx.absolute(point))
                    .collect();
                let fill = group
                    .first()
                    .map(|(index, _)| colors.fill(*index))
                    .unwrap_or_else(|| ctx.default_fill(prepared));
                paint::paint_fill(
                    &polygon,
                    paint::alpha(fill, opacity * ctx.appear),
                    ctx.window,
                );
            }

            // The fitted line: two points, at the ends of the independent
            // axis's extent.
            let ends = [(low, fit.at(low)), (high, fit.at(high))];
            let line: Vec<Point<Pixels>> = ends
                .iter()
                .map(|(independent, dependent)| {
                    ctx.absolute(if along_x {
                        point(px(*dependent as f32), px(*independent as f32))
                    } else {
                        point(px(*independent as f32), px(*dependent as f32))
                    })
                })
                .collect();
            paint::paint_stroke(&line, stroke_width, line_color, ctx.window);
        }
    });
}

/// Draw `bollingerY` (`along_x`) or `bollingerX`: a moving mean with a band at
/// `k` standard deviations either side.
pub fn draw_bollinger(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, along_x: bool) {
    let style = &prepared.mark.style;
    let window = style.window.max(1);
    let k = style.k as f64;
    // The band summarizes each series on its own, in the order of the axis it
    // runs along, so a window never spans two series or jumps back in time.
    let value_of = |row: &crate::render::PreparedRow| {
        if along_x {
            row.x_value().as_number()
        } else {
            row.y_value().as_number()
        }
    };
    let independent = |row: &crate::render::PreparedRow| {
        if along_x {
            row.y_value().clone()
        } else {
            row.x_value().clone()
        }
    };
    let mut bands: Vec<Vec<(usize, f64, f64)>> = Vec::new();
    for mut group in series_groups(prepared) {
        group.sort_by_key(|(_, row)| independent(row));
        let values: Vec<(usize, Option<f64>)> = group
            .iter()
            .map(|(index, row)| (*index, value_of(row)))
            .collect();
        // Plot's window is anchored at its end and strict by default: a datum
        // only gets a band once a full window of values precedes it.
        let band: Vec<(usize, f64, f64)> = (window - 1..values.len())
            .filter_map(|end| {
                let slice: Vec<f64> = values[end + 1 - window..=end]
                    .iter()
                    .map(|(_, value)| *value)
                    .collect::<Option<Vec<f64>>>()?;
                let mean = slice.iter().sum::<f64>() / window as f64;
                let deviation = if window > 1 {
                    (slice
                        .iter()
                        .map(|value| (value - mean).powi(2))
                        .sum::<f64>()
                        / (window - 1) as f64)
                        .sqrt()
                } else {
                    0.
                };
                Some((values[end].0, mean, deviation))
            })
            .collect();
        if band.len() >= 2 {
            bands.push(band);
        }
    }
    if bands.is_empty() {
        return;
    }

    ctx.clipped(|ctx| {
        // Plot's Bollinger band takes its mark's fill for the area and its
        // stroke for the mean line.
        let colors = crate::render::basic::Colors::resolve(prepared, ctx);
        let place = |ctx: &DrawContext<'_>, index: usize, value: f64| -> Option<Point<Pixels>> {
            let row = &prepared.rows[index];
            if along_x {
                Some(ctx.absolute(point(
                    px(ctx.frame.x.scale.position(&Value::Number(value))?),
                    px(ctx.frame.y.scale.position(row.y_value())?),
                )))
            } else {
                Some(ctx.absolute(point(
                    px(ctx.frame.x.scale.position(row.x_value())?),
                    px(ctx.frame.y.scale.position(&Value::Number(value))?),
                )))
            }
        };

        for band in &bands {
            let first = band[0].0;
            let fill_color = colors.fill(first);
            let line_color = colors.stroke(first);
            let mut upper: Vec<Point<Pixels>> = band
                .iter()
                .filter_map(|(index, mean, deviation)| place(ctx, *index, mean + k * deviation))
                .collect();
            let mut lower: Vec<Point<Pixels>> = band
                .iter()
                .filter_map(|(index, mean, deviation)| place(ctx, *index, mean - k * deviation))
                .collect();
            if upper.len() < 2 || lower.len() < 2 {
                continue;
            }
            lower.reverse();
            upper.extend(lower);
            paint::paint_fill(
                &upper,
                paint::alpha(fill_color, style.fill_opacity * ctx.appear),
                ctx.window,
            );

            let mean: Vec<Point<Pixels>> = band
                .iter()
                .filter_map(|(index, mean, _)| place(ctx, *index, *mean))
                .collect();
            paint::paint_stroke(&mean, style.stroke_width, line_color, ctx.window);
        }
    });
}

/// A least-squares fit: `dependent = intercept + slope * independent`.
struct Fit {
    slope: f64,
    intercept: f64,
    /// The mean of the independent coordinate.
    mean: f64,
    /// The sum of squared deviations of the independent coordinate.
    spread: f64,
    /// The residual standard error, when there are enough points to estimate it.
    residual: Option<f64>,
}

impl Fit {
    fn of(points: &[(f64, f64)]) -> Option<Self> {
        let n = points.len() as f64;
        let (sum_x, sum_y) = points
            .iter()
            .fold((0., 0.), |(sx, sy), (x, y)| (sx + x, sy + y));
        let (sum_xy, sum_x2) = points
            .iter()
            .fold((0., 0.), |(sxy, sx2), (x, y)| (sxy + x * y, sx2 + x * x));
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator == 0. {
            return None;
        }
        let slope = (n * sum_xy - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / n;
        let mean = sum_x / n;
        let spread = points.iter().map(|(x, _)| (x - mean).powi(2)).sum();
        let residual = (points.len() > 2).then(|| {
            let errors: f64 = points
                .iter()
                .map(|(x, y)| (y - (intercept + slope * x)).powi(2))
                .sum();
            (errors / (n - 2.)).sqrt()
        });
        Some(Self {
            slope,
            intercept,
            mean,
            spread,
            residual,
        })
    }

    fn at(&self, independent: f64) -> f64 {
        self.intercept + self.slope * independent
    }

    /// The working–Hotelling band at `ci`: the fit's standard error widened by
    /// the two-sided t quantile.
    fn confidence_band(
        &self,
        points: &[(f64, f64)],
        ci: f32,
        step: f64,
    ) -> Option<Vec<(f64, f64, f64)>> {
        let residual = self.residual?;
        if self.spread == 0. {
            return None;
        }
        let dof = points.len() as f64 - 2.;
        // The quantile is two-sided: Plot samples the band's width with its
        // magnitude, either side of the fit.
        let t = t_quantile((1. - ci as f64) / 2., dof).abs();
        let (low, high) = extent(points);
        let standard_error = |x: f64| {
            residual * (1. / points.len() as f64 + (x - self.mean).powi(2) / self.spread).sqrt()
        };
        // Sample the band across the independent axis, and always include its
        // far end so the band closes exactly at the data's extent.
        let mut samples = Vec::new();
        let mut x = low;
        while x < high {
            let error = t * standard_error(x);
            samples.push((x, self.at(x) - error, self.at(x) + error));
            x += step;
        }
        let error = t * standard_error(high);
        samples.push((high, self.at(high) - error, self.at(high) + error));
        Some(samples)
    }
}

/// The polygon a regression band fills, from its samples.
fn band_polygon(samples: &[(f64, f64, f64)], along_x: bool) -> Vec<Point<Pixels>> {
    let mut upper: Vec<Point<Pixels>> = samples
        .iter()
        .map(|(x, low, _)| {
            if along_x {
                point(px(*low as f32), px(*x as f32))
            } else {
                point(px(*x as f32), px(*low as f32))
            }
        })
        .collect();
    let mut lower: Vec<Point<Pixels>> = samples
        .iter()
        .rev()
        .map(|(x, _, high)| {
            if along_x {
                point(px(*high as f32), px(*x as f32))
            } else {
                point(px(*x as f32), px(*high as f32))
            }
        })
        .collect();
    upper.append(&mut lower);
    upper
}

fn extent(points: &[(f64, f64)]) -> (f64, f64) {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for (x, _) in points {
        low = low.min(*x);
        high = high.max(*x);
    }
    (low, high)
}

/// The two-sided Student's t quantile, the port of the `jstat` routines Plot
/// carries in `stats.js`.
fn t_quantile(p: f64, dof: f64) -> f64 {
    let x = inverse_beta(2. * p.min(1. - p), 0.5 * dof, 0.5);
    let x = (dof * (1. - x) / x).sqrt();
    if p > 0.5 { x } else { -x }
}

fn inverse_beta(p: f64, a: f64, b: f64) -> f64 {
    const EPS: f64 = 1e-8;
    if p <= 0. {
        return 0.;
    }
    if p >= 1. {
        return 1.;
    }
    let a1 = a - 1.;
    let b1 = b - 1.;
    let mut x = if a >= 1. && b >= 1. {
        let pp = if p < 0.5 { p } else { 1. - p };
        let t = (-2. * pp.ln()).sqrt();
        let mut x = (2.30753 + t * 0.27061) / (1. + t * (0.99229 + t * 0.04481)) - t;
        if p < 0.5 {
            x = -x;
        }
        let al = (x * x - 3.) / 6.;
        let h = 2. / (1. / (2. * a - 1.) + 1. / (2. * b - 1.));
        let w = (x * (al + h).sqrt()) / h
            - (1. / (2. * b - 1.) - 1. / (2. * a - 1.)) * (al + 5. / 6. - 2. / (3. * h));
        a / (a + b * (2. * w).exp())
    } else {
        let lna = (a / (a + b)).ln();
        let lnb = (b / (a + b)).ln();
        let t = (a * lna).exp() / a;
        let u = (b * lnb).exp() / b;
        let w = t + u;
        if p < t / w {
            (a * w * p).powf(1. / a)
        } else {
            1. - (b * w * (1. - p)).powf(1. / b)
        }
    };
    let afac = -gamma_ln(a) - gamma_ln(b) + gamma_ln(a + b);
    for iteration in 0..10 {
        if x == 0. || x == 1. {
            return x;
        }
        let err = beta(x, a, b) - p;
        let t = (a1 * x.ln() + b1 * (1. - x).ln() + afac).exp();
        let u = err / t;
        let step = u / (1. - 0.5 * u.min(1.) * (a1 / x - b1 / (1. - x)).min(1.));
        x -= step;
        if x <= 0. {
            x = 0.5 * (x + step);
        }
        if x >= 1. {
            x = 0.5 * (x + step + 1.);
        }
        if step.abs() < EPS * x && iteration > 0 {
            break;
        }
    }
    x
}

fn beta(x: f64, a: f64, b: f64) -> f64 {
    let bt = if x == 0. || x == 1. {
        0.
    } else {
        (gamma_ln(a + b) - gamma_ln(a) - gamma_ln(b) + a * x.ln() + b * (1. - x).ln()).exp()
    };
    if !(0. ..=1.).contains(&x) {
        return f64::NAN;
    }
    if x < (a + 1.) / (a + b + 2.) {
        bt * beta_continued(x, a, b) / a
    } else {
        1. - bt * beta_continued(1. - x, b, a) / b
    }
}

fn beta_continued(x: f64, a: f64, b: f64) -> f64 {
    const MINIMUM: f64 = 1e-30;
    let qab = a + b;
    let qap = a + 1.;
    let qam = a - 1.;
    let mut c = 1.;
    let mut d = 1. - qab * x / qap;
    if d.abs() < MINIMUM {
        d = MINIMUM;
    }
    d = 1. / d;
    let mut h = d;
    for m in 1..=100 {
        let m2 = 2. * m as f64;
        let aa = (m as f64) * (b - m as f64) * x / ((qam + m2) * (a + m2));
        d = 1. + aa * d;
        if d.abs() < MINIMUM {
            d = MINIMUM;
        }
        c = 1. + aa / c;
        if c.abs() < MINIMUM {
            c = MINIMUM;
        }
        d = 1. / d;
        h *= d * c;
        let aa = -(a + m as f64) * (qab + m as f64) * x / ((a + m2) * (qap + m2));
        d = 1. + aa * d;
        if d.abs() < MINIMUM {
            d = MINIMUM;
        }
        c = 1. + aa / c;
        if c.abs() < MINIMUM {
            c = MINIMUM;
        }
        d = 1. / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.).abs() < 3e-7 {
            break;
        }
    }
    h
}

fn gamma_ln(x: f64) -> f64 {
    const COEFFICIENTS: [f64; 6] = [
        76.18009172947146,
        -86.5053203294167,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];
    let mut series = 1.000000000190015;
    let mut y = x;
    let xx = x;
    let mut tmp = xx + 5.5;
    tmp -= (xx + 0.5) * tmp.ln();
    for coefficient in COEFFICIENTS {
        y += 1.;
        series += coefficient / y;
    }
    (2.506628274631 * series / xx).ln() - tmp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_t_quantile_matches_the_table() {
        // The 97.5% quantile of a t distribution with a few degrees of freedom,
        // the values Plot's `ci: 0.95` bands use.
        assert!((t_quantile(0.025, 5.) + 2.5706).abs() < 1e-3);
        assert!((t_quantile(0.025, 10.) + 2.2281).abs() < 1e-3);
        assert!((t_quantile(0.025, 1000.) + 1.9623).abs() < 1e-2);
    }

    #[test]
    fn a_band_straddles_the_fit() {
        // Points scattered around a downward line, as a screen-space fit sees
        // them: the band must sit either side of the fit, not above it.
        let points: Vec<(f64, f64)> = (0..120)
            .map(|index| {
                let x = 100. + index as f64 * 10.;
                let noise = if index % 2 == 0 { 8. } else { -8. };
                (x, 690. - index as f64 * 4.4 + noise)
            })
            .collect();
        let fit = Fit::of(&points).unwrap();
        let band = fit.confidence_band(&points, 0.95, 4.).unwrap();
        let (_, low, high) = band[band.len() / 2];
        let x = band[band.len() / 2].0;
        assert!(
            low < fit.at(x) && high > fit.at(x),
            "band {low}..{high} at {x}, fit {}",
            fit.at(x)
        );
        let (_, low, high) = *band.last().unwrap();
        let width = high - low;
        assert!(width < 200., "band too wide at the end: {width}");
    }

    #[test]
    fn a_fit_recovers_a_line() {
        let points: Vec<(f64, f64)> = (0..10).map(|x| (x as f64, 2. * x as f64 + 3.)).collect();
        let fit = Fit::of(&points).unwrap();
        assert!((fit.slope - 2.).abs() < 1e-9);
        assert!((fit.intercept - 3.).abs() < 1e-9);
    }
}
