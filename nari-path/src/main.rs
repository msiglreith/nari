use std::collections::VecDeque;

use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use nari_vello::{
    kurbo::{BezPath, Line, PathEl, PathSeg, Point, QuadBez, Rect, Vec2},
    peniko::Color,
};
use softbuffer::GraphicsContext;

const SUBDIVISION: usize = 7;
const GRID_SAMPLES: usize = SUBDIVISION + 1;
const SAMPLES: usize = 4;

type Path = Vec<PathSeg>;

fn path_from_elem(elements: Vec<PathEl>) -> Path {
    BezPath::from_vec(elements).segments().collect()
}

struct CoarseTile {
    // origin
    tx: usize,
    ty: usize,

    level: usize,

    tape: Vec<Path>,
}

struct FineTile {
    x: usize,
    y: usize,
    tape: Vec<Path>,
}

pub const fn rgb_u8(r: u32, g: u32, b: u32) -> u32 {
    b | (g << 8) | (r << 16)
}

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

fn make_diamond(cx: f64, cy: f64, size: f64) -> Path {
    path_from_elem(vec![
        PathEl::MoveTo(Point::new(cx, cy - size)),
        PathEl::LineTo(Point::new(cx + size, cy)),
        PathEl::LineTo(Point::new(cx, cy + size)),
        PathEl::LineTo(Point::new(cx - size, cy)),
        PathEl::ClosePath,
    ])
}

const COLOR_LEVELS: [u32; 5] = [
    rgb_u8(0xff, 0xb7, 0xc3),
    rgb_u8(0xd3, 0xfa, 0xc7),
    rgb_u8(0xd9, 0xf2, 0xb4),
    rgb_u8(0xb4, 0xeb, 0xca),
    rgb_u8(0xbc, 0xf4, 0xf5),
];

fn line_coeff(p: Point, p0: Point, p1: Point) -> (Vec2, Vec2) {
    let a1 = p1 - p0;
    let a0 = p0 - p;

    (a0, a1)
}

fn quad_coeff(p: Point, p0: Point, p1: Point, p2: Point) -> (Vec2, Vec2, Vec2) {
    let a2 = p0.to_vec2() - 2.0 * p1.to_vec2() + p2.to_vec2();
    let a1 = 2.0 * p1.to_vec2() - 2.0 * p0.to_vec2();
    let a0 = p0 - p;

    (a0, a1, a2)
}

fn cubic_coeff(p: Point, p0: Point, p1: Point, p2: Point, p3: Point) -> (Vec2, Vec2, Vec2, Vec2) {
    let a3 = 3.0 * p1.to_vec2() - 3.0 * p2.to_vec2() + p3.to_vec2() - p0.to_vec2();
    let a2 = 3.0 * p0.to_vec2() - 6.0 * p1.to_vec2() + 3.0 * p2.to_vec2();
    let a1 = 3.0 * p1.to_vec2() - 3.0 * p0.to_vec2();
    let a0 = p0 - p;

    (a0, a1, a2, a3)
}

fn line_norm_sqr(t: f64, a0: Vec2, a1: Vec2) -> f64 {
    let c = a1 * t + a0;
    c.dot(c)
}

fn quad_norm_sqr(t: f64, a0: Vec2, a1: Vec2, a2: Vec2) -> f64 {
    let c = a0 + (a1 + a2 * t) * t;
    c.dot(c)
}

fn cubic_norm_sqr(t: f64, a0: Vec2, a1: Vec2, a2: Vec2, a3: Vec2) -> f64 {
    let c = a0 + (a1 + (a2 + a3 * t) * t) * t;
    c.dot(c)
}

fn line_iteration(t: f64, a0: Vec2, a1: Vec2) -> f64 {
    let d0 = a1 * t + a0;
    let d1 = a1;

    let df = |t: f64| 2.0 * d1.dot(d0);
    let ddf = |t: f64| 2.0 * d1.dot(d1);

    // let xdf = df(t);
    // let xddf = ddf(t);
    // t - xdf / xddf

    t - a1.dot(d0) / a1.dot(a1)
}

fn quad_iteration(t: f64, a0: Vec2, a1: Vec2, a2: Vec2) -> f64 {
    let d0 = a0 + (a1 + a2 * t) * t;
    let d1 = a1 + 2.0 * a2 * t;
    let d2 = 2.0 * a2;

    let df = |t: f64| 2.0 * d1.dot(d0);
    let ddf = |t: f64| 2.0 * (d2.dot(d0) + (d1).dot(d1));

    // let xdf = df(t);
    // let xddf = ddf(t);
    // t - xdf / xddf

    t - d0.dot(d1) / (a1.dot(d0) + d1.dot(d1))
}

fn cubic_iteration(t: f64, a0: Vec2, a1: Vec2, a2: Vec2, a3: Vec2) -> f64 {
    let d0 = a0 + (a1 + (a2 + a3 * t) * t) * t;
    let d1 = a1 + (2.0 * a2 + 3.0 * a3 * t) * t;
    let d2 = 2.0 * a2 + 6.0 * a3 * t;

    let df = |t: f64| 2.0 * (d1).dot(d0);
    let ddf = |t: f64| {
        3.0 * (a1.dot(a0 + a1 * t + a2 * t * t) + (a1 + 2.0 * a2 * t).dot(a1 + 2.0 * a2 * t))
    };

    let df = |t: f64| 2.0 * d1.dot(d0);
    let ddf = |t: f64| 2.0 * (d2.dot(d0) + (d1).dot(d1));

    // let xdf = df(t);
    // let xddf = ddf(t);
    // t - xdf / xddf

    t - d0.dot(d1) / (d2.dot(d0) + d1.dot(d1))
}

fn draw(width: u32, height: u32) -> Vec<u32> {
    let mut framebuffer = vec![0u32; (width * height) as usize];

    // clearing
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            framebuffer[index] = rgb(1.0, 1.0, 1.0);
        }
    }

    let p0 = Point::new(100.0, 100.0);
    let p1 = Point::new(200.0, 250.0);
    let p2 = Point::new(300.0, 350.0);
    let p3 = Point::new(700.0, 400.0);

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;

            let p = Point::new(x as f64, y as f64);

            let mut t = 0.5;

            // if p.y < p0.y.min(p2.y) || p.y > p0.y.max(p2.y) {
            //     continue;
            // }

            // let (a0, a1) = line_coeff(p, p0, p1);
            // for i in 0..5 {
            //     t = line_iteration(t, a0, a1);
            // }
            // let distance = line_norm_sqr(t.clamp(0.0, 1.0), a0, a1).sqrt();

            let (a0, a1, a2) = quad_coeff(p, p0, p1, p2);
            for i in 0..4 {
                t = quad_iteration(t, a0, a1, a2);
            }
            let distance = quad_norm_sqr(t.clamp(0.0, 1.0), a0, a1, a2).sqrt();

            // let (a0, a1, a2, a3) = cubic_coeff(p, p0, p1, p2, p3);
            // for i in 0..4 {
            //     t = cubic_iteration(t, a0, a1, a2, a3);
            // }
            // let distance = cubic_norm_sqr(t.clamp(0.0, 1.0), a0, a1, a2, a3).sqrt();

            let c = 0.8 + 0.2 * (distance).cos();
            let coverage = c;

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
