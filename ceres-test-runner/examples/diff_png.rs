//! Debug helper: pixel-diff two PNGs, printing mismatched coordinates grouped by row.
//!
//! Usage: `cargo run -p ceres-test-runner --example diff_png -- <expected.png> <actual.png>`
//!
//! NOTE: the first argument is the expected/reference image.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = image::open(&args[1]).unwrap().to_rgba8();
    let b = image::open(&args[2]).unwrap().to_rgba8();
    assert_eq!(a.dimensions(), b.dimensions());

    let mut total = 0usize;
    for y in 0..144 {
        let mut row_mismatches: Vec<(u32, [u8; 4], [u8; 4])> = Vec::new();
        for x in 0..160 {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            if pa != pb {
                row_mismatches.push((x, pa, pb));
            }
        }
        if !row_mismatches.is_empty() {
            total += row_mismatches.len();
            println!("y={:3} ({} px):", y, row_mismatches.len());
            for (x, pa, pb) in &row_mismatches {
                println!("    x={x:3} expected={pa:?} got={pb:?}");
            }
        }
    }
    println!("total mismatched pixels: {total}");
}
