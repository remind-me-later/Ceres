//! Debug helper: diffs two PNGs pixel for pixel, printing the mismatches row
//! by row.
//!
//! Usage: `cargo run -p ceres-test-runner --example diff_png -- <expected.png> <actual.png>`
//!
//! The comparison is exact, like `ExactScreenshotCheck`; the suites that use
//! `RankedScreenshotCheck` accept images that differ only in their palette.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [expected, actual] = args.as_slice() else {
        eprintln!("usage: diff_png <expected.png> <actual.png>");
        std::process::exit(2);
    };
    let a = image::open(expected).expect("expected image").to_rgba8();
    let b = image::open(actual).expect("actual image").to_rgba8();
    assert_eq!(a.dimensions(), b.dimensions(), "the sizes differ");

    let (width, height) = a.dimensions();
    let mut total = 0_usize;
    for y in 0..height {
        let row: Vec<_> = (0..width)
            .map(|x| (x, a.get_pixel(x, y).0, b.get_pixel(x, y).0))
            .filter(|(_, pa, pb)| pa != pb)
            .collect();
        if !row.is_empty() {
            total += row.len();
            println!("y={y:3} ({} px):", row.len());
            for (x, pa, pb) in &row {
                println!("    x={x:3} expected={pa:?} got={pb:?}");
            }
        }
    }
    println!("total mismatched pixels: {total}");
}
