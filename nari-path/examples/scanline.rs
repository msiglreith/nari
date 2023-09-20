#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
struct vec2 {
    x: f32,
    y: f32,
}
const SAMPLE_LOCATIONS: [i32; 16] = [5, 1, 4, 6, 0, 4, 2, 7, 13, 9, 12, 14, 8, 12, 10, 15];

fn main() {
    dbg!(1.0 / 16.0);
    let p1 = vec2 { x: 0.0, y: 0.0 };
    let p0 = vec2 { x: 2.0, y: 4.0 };

    let (y0, y1, mut x, dx) = if p0.y < p1.y {
        let dx = (p1.x - p0.x) / (p1.y - p0.y);
        let py = p0.y * 8.0 - 0.5;
        let y0 = (p0.y * 8.0 - 0.5).ceil();
        let y1 = (p1.y * 8.0 - 0.5).ceil();
        let x = p0.x * 8.0 - 0.5 + (y0 - py) * dx;
        (y0, y1, x, dx)
    } else {
        let dx = (p0.x - p1.x) / (p0.y - p1.y);
        let py = p1.y * 8.0 - 0.5;
        let y0 = (p1.y * 8.0 + 0.5).floor();
        let y1 = (p0.y * 8.0 + 0.5).floor();
        let x = p1.x * 8.0 - 0.5 + (y0 - py) * dx;
        (y0, y1, x, dx)
    };

    dbg!(y0, x, dx);

    let mut mask = 0u32;

    let mut prev_x = y0 as i32 / 16;
    let mut prev_y = x as i32 / 16;

    for y in y0 as i32..y1 as i32 {
        let ty = y / 16;
        let tx = x as i32 / 16;
        let sy = y % 16;
        let sx = x as i32 % 16;

        {
            if ty != prev_y {
                println!("my: {:?} {:b}", (prev_x, prev_y), mask);
                mask = 0;
            } else if tx != prev_x {
                let y_mask = ((1 << sy) - 1) | ((1 << (sy + 16)) - 1);
                mask ^= y_mask;
                println!("mx: {:?} {:b}", (prev_x, prev_y), mask);
                mask = 0;
            }
        }

        let loc = SAMPLE_LOCATIONS[sy as usize];
        if loc <= sx {
            mask |= 1 << sy;
        }
        if loc <= sx + 8 {
            mask |= 1 << (sy + 16);
        }

        println!("{:?}", (tx, ty, sx, sy));

        x += dx;
        prev_x = tx;
        prev_y = ty;
    }

    if mask != 0 {
        println!("mx: {:?} {:b}", (prev_x, prev_y), mask);
        mask = 0;
    }
}
