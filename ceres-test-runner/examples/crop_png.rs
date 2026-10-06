//! Debug helper: crop a region of a PNG and magnify it.
//!
//! Usage: `cargo run -p ceres-test-runner --example crop_png -- <in.png> <x0> <y0> <x1> <y1> <scale> <out.png>`

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let img = image::open(&args[1]).unwrap().to_rgba8();
    let (x0, y0, x1, y1, scale): (u32, u32, u32, u32, u32) = (
        args[2].parse().unwrap(),
        args[3].parse().unwrap(),
        args[4].parse().unwrap(),
        args[5].parse().unwrap(),
        args[6].parse().unwrap(),
    );
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
    out.save(&args[7]).unwrap();
    println!("wrote {}", args[7]);
}
