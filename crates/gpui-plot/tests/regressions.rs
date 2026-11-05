//! Regression tests for edge cases found in the library-wide review: number
//! formatting, tick generation, scale transforms, projections and transforms.

use gpui_plot::scale::BandScale;
use gpui_plot::ticks::{format_linear_tick, linear_ticks, time_ticks};
use gpui_plot::transform::{self, Thresholds};
use gpui_plot::value::format_number;
use gpui_plot::{
    ContinuousScale, Dataset, Dodge, DodgeAnchor, Projection, Select, Selector, StackOffset, Table,
    Transform, Value,
};

#[test]
fn whole_numbers_keep_their_zeros() {
    assert_eq!(format_number(1000.4), "1000");
    assert_eq!(format_number(150.2), "150");
    assert_eq!(format_number(12.04), "12");
    assert_eq!(format_number(2.5e15), "2500000000000000");
    assert_eq!(format_number(-0.0004), "0");
    assert_eq!(format_number(1.50), "1.5");
}

#[test]
fn large_ticks_do_not_saturate() {
    assert_eq!(&*format_linear_tick(1e20, 1e19), "100000000000000000000");
}

#[test]
fn ticks_terminate_when_the_step_is_below_an_ulp() {
    let ticks = linear_ticks([1e16, 1e16 + 4.], 5, Transform::Linear);
    assert!(!ticks.is_empty() && ticks.len() < 1_000);
    let ticks = linear_ticks([-f64::MAX, f64::MAX], 5, Transform::Linear);
    assert!(ticks.len() <= 1_000);
    assert!(linear_ticks([f64::NAN, 1.], 5, Transform::Linear).is_empty());
}

#[test]
fn log_ticks_label_both_decades() {
    let ticks = linear_ticks([10., 100.], 10, Transform::Log);
    assert_eq!(ticks.first(), Some(&10.));
    assert_eq!(ticks.last(), Some(&100.));
}

const DAY: f64 = 86_400_000.;

#[test]
fn dense_time_ticks_cover_the_whole_domain() {
    // 2020-01-01 .. 2021-01-01
    let (lo, hi) = (1_577_836_800_000., 1_609_459_200_000.);
    let ticks = time_ticks([lo, hi], 400);
    assert!(ticks.len() <= 64);
    assert!(*ticks.last().unwrap() > hi - 31. * DAY, "{ticks:?}");
}

#[test]
fn six_week_time_axis_has_more_than_two_ticks() {
    let lo = 1_577_836_800_000.; // 2020-01-01, a Wednesday
    let ticks = time_ticks([lo, lo + 40. * DAY], 6);
    assert!(ticks.len() >= 3, "{ticks:?}");
    // Weekly ticks fall on Sundays: 2020-01-05 is one.
    let sunday = 1_578_182_400_000.;
    assert!(
        ticks
            .iter()
            .all(|t| ((t - sunday) / DAY).rem_euclid(7.) == 0.)
    );
}

#[test]
fn year_ticks_step_by_nice_counts() {
    // 2000-01-01 .. 2007-06-01, two ticks requested
    let ticks = time_ticks([946_684_800_000., 1_180_656_000_000.], 2);
    assert!(ticks.len() >= 2, "{ticks:?}");
}

#[test]
fn pow_and_sqrt_scale_negative_domains() {
    let pow = ContinuousScale::new(Transform::Pow, [-10., 10.], [0., 100.]).with_exponent(2.);
    assert!(pow.position(5.).is_some_and(|p| p > 50.));
    assert!((pow.invert(50.)).abs() < 1e-9);
    let sqrt = ContinuousScale::new(Transform::Sqrt, [-10., 10.], [0., 100.]);
    assert!(sqrt.position(-5.).is_some_and(|p| p < 50.));
    let zero = ContinuousScale::new(Transform::Pow, [0., 10.], [0., 100.]).with_exponent(0.);
    assert_eq!(zero.position(5.), Some(50.));
}

#[test]
fn clamped_scales_invert_inside_the_domain() {
    let scale = ContinuousScale::new(Transform::Linear, [0., 10.], [0., 100.]).clamped(true);
    assert_eq!(scale.invert(200.), 10.);
}

#[test]
fn capped_bands_are_centred_in_their_slot() {
    let keys = ["a", "b", "c"].map(|k| Value::text(k).key());
    let scale = BandScale::new(keys.clone(), [0., 300.]).max_band_width(10.);
    assert_eq!(scale.center(&keys[1]), Some(150.));
}

#[test]
fn non_finite_values_stay_out_of_extents() {
    let extent = gpui_plot::scale::extent([Value::Number(f64::NAN), Value::Number(2.)]);
    assert_eq!(extent, Some([2., 2.]));
    let extent = gpui_plot::scale::extent([Value::Number(f64::INFINITY), Value::Number(1.)]);
    assert_eq!(extent, Some([1., 1.]));
}

#[test]
fn nan_numbers_have_a_total_order() {
    let mut values = [
        Value::Number(3.),
        Value::Number(f64::NAN),
        Value::Number(1.),
        Value::Number(2.),
    ];
    values.sort();
    assert_eq!(values[0], Value::Number(1.));
    assert_eq!(values[2], Value::Number(3.));
    assert_eq!(values[3], Value::Number(f64::NAN));
}

#[test]
fn mercator_survives_poles_and_the_antimeridian() {
    let world = Projection::mercator(-180., 180., 90., -90.);
    let [_, top] = world.project(0., 60.);
    let [_, bottom] = world.project(0., -60.);
    assert!(top < bottom, "north above south: {top} {bottom}");
    let pacific = Projection::mercator(170., -170., 10., -10.);
    let [left, _] = pacific.project(175., 0.);
    let [right, _] = pacific.project(-175., 0.);
    assert!(left < 0.5 && right > 0.5, "{left} {right}");
}

fn numbers(field: &str, values: &[f64]) -> Dataset {
    Dataset::new(
        values
            .iter()
            .map(|v| Table::empty().with(field, Value::Number(*v))),
    )
}

#[test]
fn bins_skip_nan_and_reject_bad_widths() {
    let data = numbers("v", &[f64::NAN, 1., 2.]);
    let (bins, _) = transform::bin(&data, "v", Thresholds::Count(2));
    assert_eq!(bins.iter().map(|b| b.rows.len()).sum::<usize>(), 2);
    // A zero or NaN interval falls back to the automatic thresholds.
    for width in [0., -1., f64::NAN, 1e-300] {
        let (bins, _) = transform::bin(&data, "v", Thresholds::Interval(width));
        assert!(!bins.is_empty() && bins.len() <= 10_000);
    }
    let data = numbers("v", &[f64::INFINITY, 1., 2.]);
    let (bins, _) = transform::bin(&data, "v", Thresholds::Auto);
    assert!(bins.len() < 100);
}

#[test]
fn negative_values_stack_below_zero() {
    let data = Dataset::new([Table::empty()
        .with("x", Value::text("a"))
        .with("s1", Value::Number(5.))
        .with("s2", Value::Number(-3.))]);
    let series = transform::stack_wide(&data, "x", &["s1", "s2"], StackOffset::Zero);
    assert_eq!((series[0].points[0].y0, series[0].points[0].y1), (0., 5.));
    assert_eq!((series[1].points[0].y0, series[1].points[0].y1), (0., -3.));
    let series = transform::stack_wide(&data, "x", &["s1", "s2"], StackOffset::Normalize);
    let (s1, s2) = (&series[0].points[0], &series[1].points[0]);
    assert!(s1.y1 <= 1. && s2.y1 >= 0. && s2.y1 <= s1.y0 + 1e-9);
}

#[test]
fn a_start_dodge_stays_on_its_side() {
    let data = numbers("x", &[50., 50., 50.]);
    let positions = [Some(50.), Some(50.), Some(50.)];
    let config = Dodge {
        anchor: DodgeAnchor::Start,
        ..Dodge::default()
    };
    let packed = transform::dodge(&data, &positions, "d", config, [0., 100.]);
    for row in packed.rows() {
        assert!(row.number("d").unwrap() >= 0., "{:?}", row.number("d"));
    }
}

#[test]
fn select_max_keeps_the_first_tie() {
    let data = Dataset::new(["a", "b"].map(|name| {
        Table::empty()
            .with("name", Value::text(name))
            .with("v", Value::Number(5.))
    }));
    let picked = transform::select(
        &data,
        &Select {
            selector: Selector::Max("v".into()),
            group: None,
        },
    );
    assert_eq!(picked.rows()[0].value("name"), Value::text("a"));
}
