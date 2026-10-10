//! Debug helper: crop a region of a PNG and magnify it.
//!
//! Usage: `cargo run -p ceres-test-runner --example crop_png -- <in.png> <x0> <y0> <x1> <y1> <scale> <out.png>`

const USAGE: &str = "usage: crop_png <in.png> <x0> <y0> <x1> <y1> <scale> <out.png>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, x0, y0, x1, y1, scale, output] = args.as_slice() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let numbers: Result<Vec<u32>, _> = [x0, y0, x1, y1, scale].iter().map(|v| v.parse()).collect();
    let Ok(&[x0, y0, x1, y1, scale]) = numbers.as_deref() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let img = image::open(input).expect("input image").to_rgba8();
    let w = (x1 - x0) * scale;
    let h = (y1 - y0) * scale;
    let mut out = image::ImageBuffer::from_pixel(w, h, image::Rgba([40, 40, 40, 255]));
    for y in y0..y1 {
        for x in x0..x1 {
            let p = *img.get_pixel(x, y);
            for dy in 0..scale {
                for dx in 0..scale {
                    out.put_pixel((x - x0) * scale + dx, (y - y0) * scale + dy, p);
                }
            }
        }
    }
    out.save(output).expect("save PNG");
    println!("wrote {output}");
}
