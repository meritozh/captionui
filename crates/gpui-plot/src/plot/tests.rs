//! Tests that span several of the plot's submodules.

use crate::plot::{Plot, Prepared};
use crate::prelude::*;
use crate::table::{Dataset, Table};
use crate::value::Value;

/// A pipeline result as the dump harness prints it.
fn dump_prepared((grid, facets, populated): &Prepared) -> String {
    let mut out = String::new();
    crate::debug_dump::grid(&mut out, grid, Some(populated));
    for (cell, marks) in facets.iter().enumerate() {
        out.push_str(&format!("facet {cell}\n"));
        for (index, mark) in marks.iter().enumerate() {
            crate::debug_dump::mark(&mut out, index, mark);
            for (i, row) in mark.rows.iter().enumerate() {
                crate::debug_dump::row(&mut out, i, row);
            }
        }
    }
    out
}

#[test]
fn natural_height_hands_back_the_marks_it_measured() {
    let data = || {
        (0..40).map(|i| {
            Table::empty()
                .with("x", Value::Number(i as f64 * 0.37))
                .with("y", Value::Number((i * 7 % 11) as f64))
                .with("g", Value::text(["a", "b", "c"][i % 3]))
        })
    };
    let plots = [
        Plot::new(data()).dot(|m| m.x("x").y("y").fill("g")),
        Plot::new(data()).fx("g").title("faceted").mark(
            crate::marks::rect_y(Dataset::new(data()))
                .x("x")
                .bin_output("y", crate::GroupReducer::Count),
        ),
        Plot::new(data())
            .aspect_ratio(1.)
            .line(|m| m.x("x").y("y").stroke("y")),
    ];
    for plot in &plots {
        let (height, prepared) = plot.natural_height_prepared(640.);
        assert_eq!(height.to_bits(), plot.natural_height(640.).to_bits());
        assert_eq!(
            dump_prepared(&prepared),
            dump_prepared(&plot.prepare_marks())
        );
        // Primed, the next resolve takes it once; the one after runs the
        // pipeline again.
        plot.prime(prepared);
        assert!(plot.primed.take().is_some());
        assert!(plot.primed.borrow().is_none());
    }
    // A retained plot primes its first frame from the height it measures.
    let retained = Plot::new(data()).dot(|m| m.x("x").y("y")).retained();
    let height = retained.natural_height(640.);
    retained.with_plot(|plot| {
        assert_eq!(height.to_bits(), plot.natural_height(640.).to_bits());
        let primed = plot.primed.take().expect("primed before the first frame");
        assert_eq!(dump_prepared(&primed), dump_prepared(&plot.prepare_marks()));
    });
}
