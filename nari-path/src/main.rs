use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use nari_vello::{
    kurbo::{Point, QuadBez, Rect, Vec2},
    peniko::Color,
};
use softbuffer::GraphicsContext;

pub fn rgb(r: f64, g: f64, b: f64) -> u32 {
    let r = (255.0 * r.clamp(0.0, 1.0)) as u32;
    let g = (255.0 * g.clamp(0.0, 1.0)) as u32;
    let b = (255.0 * b.clamp(0.0, 1.0)) as u32;

    b | (g << 8) | (r << 16)
}

fn quad_winding(q: QuadBez, p: Point) -> isize {
    let bbox = Rect::from_points(q.p0, q.p2);

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

    if !bbox.contains(p) {
        return 0;
    }

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

    let background = rgb(1.0, 1.0, 1.0);
    let foreground = rgb(0.0, 0.0, 0.0);

    for y in 0..height {
        for x in 0..width {
            let p = Point::new(x as _, y as _);
            let index = (y * width + x) as usize;

            framebuffer[index] = background;

            for quad in [q0, q1, q2, q3] {
                let winding = quad_winding(quad, p);
                if winding == 1 {
                    framebuffer[index] = rgb(0.0, 1.0, 0.0);
                } else if winding == -1 {
                    framebuffer[index] = rgb(1.0, 0.0, 0.0);
                } else if winding == 2 {
                    framebuffer[index] = rgb(0.0, 0.0, 1.0);
                }
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
