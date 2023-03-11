use std::collections::VecDeque;

use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use nari_vello::{
    kurbo::{BezPath, Line, PathEl, PathSeg, Point, QuadBez, Rect, Vec2},
    peniko::Color,
};
use softbuffer::GraphicsContext;

const SUBDIVISION: usize = 7;
const SAMPLES: usize = 4;

type Path = Vec<PathSeg>;

struct CoarseTile {
    // origin
    tx: usize,
    ty: usize,

    level: usize,

    tape: Vec<Path>,
}

struct FineTile {}

pub fn rgb(r: f64, g: f64, b: f64) -> u32 {
    let r = (255.0 * r.clamp(0.0, 1.0)) as u32;
    let g = (255.0 * g.clamp(0.0, 1.0)) as u32;
    let b = (255.0 * b.clamp(0.0, 1.0)) as u32;

    b | (g << 8) | (r << 16)
}

fn quad_winding(q: QuadBez, p: Point) -> isize {
    if p.y < q.p0.y.min(q.p2.y) || p.y > q.p0.y.max(q.p2.y) {
        return 0;
    }

    let p0p1 = q.p1 - q.p0;
    let p0p2 = q.p2 - q.p0;
    let p0p = p - q.p0;

    let wp = p0p2.cross(p0p);
    let w1 = p0p2.cross(p0p1);

    let t0 = (q.p1 - p).cross(q.p2 - p);
    let t1 = (p - q.p0).cross(q.p2 - q.p0);
    let t2 = (q.p1 - q.p0).cross(p - q.p0);

    let dy = q.p2.y - q.p0.y;
    let w = if dy > 0.0 { -1 } else { 1 };

    // p1 and sample different side of p0p1
    let skip_check = wp * w1 < 0.0;
    if skip_check {
        let is_left = wp * dy >= 0.0;
        return if is_left { w } else { 0 };
    }

    let is_inside = t1 * t1 - 4.0 * t0 * t2;
    if is_inside * dy * w1 >= 0.0 {
        w
    } else {
        0
    }
}

fn div_align_up(x: u32, y: u32) -> u32 {
    (x + y - 1) / y
}

fn make_diamond(cx: f64, cy: f64, size: f64) -> [PathEl; 5] {
    [
        PathEl::MoveTo(Point::new(cx, cy - size)),
        PathEl::LineTo(Point::new(cx + size, cy)),
        PathEl::LineTo(Point::new(cx, cy + size)),
        PathEl::LineTo(Point::new(cx - size, cy)),
        PathEl::ClosePath,
    ]
}

fn draw(width: u32, height: u32) -> Vec<u32> {
    let mut framebuffer = vec![0u32; (width * height) as usize];

    let q0 = QuadBez {
        p0: Point::new(100.0, 50.0),
        p1: Point::new(300.0, 100.0),
        p2: Point::new(500.0, 350.0),
    };
    let q1 = QuadBez {
        p0: Point::new(1000.0, 350.0),
        p1: Point::new(800.0, 100.0),
        p2: Point::new(600.0, 50.0),
    };
    let q2 = QuadBez {
        p0: Point::new(100.0, 400.0),
        p1: Point::new(300.0, 650.0),
        p2: Point::new(500.0, 700.0),
    };
    let q3 = QuadBez {
        p0: Point::new(1000.0, 700.0),
        p1: Point::new(800.0, 650.0),
        p2: Point::new(600.0, 400.0),
    };

    let paths = vec![BezPath::from_vec(make_diamond(200.0, 150.0, 70.0).to_vec())
        .segments()
        .collect::<Vec<_>>()];

    // clearing
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            framebuffer[index] = rgb(1.0, 1.0, 1.0);
        }
    }

    // let mut coarse_tiles = VecDeque::default();
    // coarse_tiles.push_back(CoarseTile {
    //     tx: 0,
    //     ty: 0,
    //     level: 4,
    //     tape: vec![q0, q1, q2, q3],
    // });

    // while let Some(tile) = coarse_tiles.pop_front() {
    //     let dx = SUBDIVISION.pow(tile.level as u32 - 1);
    //     let tile_size = SUBDIVISION.pow(tile.level as u32);
    //     let x0 = tile.tx * tile_size;
    //     let y0 = tile.ty * tile_size;

    //     let mut winding = [[0; SAMPLES]; SAMPLES];

    //     for sy in 0..SAMPLES {
    //         for sx in 0..SAMPLES {
    //             let mut subtile = CoarseTile {
    //                 tx: tile.tx * SUBDIVISION + sx,
    //                 ty: tile.ty * SUBDIVISION + sy,
    //                 level: tile.level - 1,
    //                 tape: Vec::default(),
    //             };
    //             let p = Point::new((x0 + sx * dx) as f64, (y0 + sy * dx) as f64);
    //             for segment in &tile.tape {
    //                 let local = quad_winding(*segment, p);
    //                 winding[sy as usize][sx as usize] += quad_winding(*segment, p);
    //             }
    //         }
    //     }
    // }

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;

            framebuffer[index] = rgb(0.0, 0.0, 0.0);

            let mut coverage = 0.0;

            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let p = Point::new(
                        x as f64 + sx as f64 / (SAMPLES + 1) as f64,
                        y as f64 + sy as f64 / (SAMPLES + 1) as f64,
                    );

                    for path in &paths {
                        let mut winding = 0;

                        for segment in path {
                            match segment.clone() {
                                PathSeg::Line(Line { p0, p1 }) => {
                                    let y_min = p0.y.min(p1.y);
                                    let y_max = p0.y.max(p1.y);

                                    if p.y < y_min || p.y > y_max {
                                        // no covering of point sample
                                        continue;
                                    }

                                    // orientation
                                    let dy = p1.y - p0.y;
                                    let w = if dy > 0.0 { 1 } else { -1 };

                                    let p0p1 = p1 - p0;
                                    let p0p = p - p0;

                                    let wp = p0p1.cross(p0p);

                                    let is_left = wp * dy >= 0.0;
                                    if is_left {
                                        winding += w;
                                    }
                                }
                                _ => unimplemented!(),
                            }
                        }

                        coverage += winding as f64 / (SAMPLES * SAMPLES) as f64;
                    }
                }
            }

            if coverage > 0.0 {
                framebuffer[index] = rgb(0.0, coverage, 0.0);
            } else if coverage < 0.0 {
                framebuffer[index] = rgb(-coverage, 0.0, 0.0);
            }
        }
    }

    framebuffer
}

fn main() {
    let platform = Platform::new();

    let mut surface =
        unsafe { GraphicsContext::new(&platform.surface, &platform.surface) }.unwrap();

    platform.run(move |event_loop, event| {
        match event {
            Event::Paint => {
                let Extent { width, height } = event_loop.surface.extent();
                let buffer = draw(width, height);
                surface.set_buffer(&buffer, width as u16, height as u16);
            }

            Event::Hittest { x, y, area } => {
                const MARGIN: i32 = 5;
                const CAPTION_HEIGHT: i32 = 28;

                let Extent { width, height } = event_loop.surface.extent();

                let w = width as i32;
                let h = height as i32;

                *area = match (x, y) {
                    (_, 0..=CAPTION_HEIGHT) => SurfaceArea::Caption,
                    _ => SurfaceArea::Client,
                };

                if !event_loop.surface.is_maximized() {
                    // resize border
                    *area = match (x, y) {
                        _ if x <= MARGIN && y <= MARGIN => SurfaceArea::TopLeft,
                        _ if x >= w - MARGIN && y <= MARGIN => SurfaceArea::TopRight,
                        _ if x >= w - MARGIN && y >= h - MARGIN => SurfaceArea::BottomRight,
                        _ if x <= MARGIN && y >= h - MARGIN => SurfaceArea::BottomLeft,
                        _ if x <= MARGIN => SurfaceArea::Left,
                        _ if y <= MARGIN => SurfaceArea::Top,
                        _ if x >= w - MARGIN => SurfaceArea::Right,
                        _ if y >= h - MARGIN => SurfaceArea::Bottom,
                        _ => *area,
                    };
                }
            }

            _ => (),
        }
        ControlFlow::Continue
    });
}
