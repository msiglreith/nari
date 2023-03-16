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

    let paths = vec![make_diamond(200.0, 150.0, 70.0)];

    // clearing
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            framebuffer[index] = rgb(1.0, 1.0, 1.0);
        }
    }

    let mut coarse_tiles = VecDeque::default();
    coarse_tiles.push_back(CoarseTile {
        tx: 0,
        ty: 0,
        level: 4,
        tape: paths,
    });

    let mut fine_tiles = Vec::new();

    while let Some(tile) = coarse_tiles.pop_front() {
        let dx = SUBDIVISION.pow(tile.level as u32 - 1);
        let tile_size = SUBDIVISION.pow(tile.level as u32);
        let x0 = tile.tx * tile_size;
        let y0 = tile.ty * tile_size;

        // let mut winding = [[0; GRID_SAMPLES]; GRID_SAMPLES];

        for sy in 0..SUBDIVISION {
            for sx in 0..SUBDIVISION {
                let x = x0 + sx * dx;
                let y = y0 + sy * dx;

                if x >= width as usize || y >= height as usize {
                    continue;
                }

                let mut backdrop = 0;
                let mut new_tape = Vec::default();
                let p = Point::new(x as f64, y as f64);

                let corners = [
                    Point::new(x as f64, y as f64),
                    Point::new((x + dx) as f64, y as f64),
                    Point::new(x as f64, (y + dx) as f64),
                    Point::new((x + dx) as f64, (y + dx) as f64),
                ];

                for path in &tile.tape {
                    let mut new_path = Vec::default();
                    for segment in path {
                        let mut winding = [0; 4];
                        match segment.clone() {
                            PathSeg::Line(Line { p0, p1 }) => {
                                let y_min = p0.y.min(p1.y);
                                let y_max = p0.y.max(p1.y);

                                if y_max < y as f64 || ((y + dx) as f64) < y_min {
                                    continue;
                                }

                                let dy = p1.y - p0.y;
                                if dy == 0.0 {
                                    continue;
                                }

                                let w = if dy > 0.0 { 1 } else { -1 };

                                let p0p1 = p1 - p0;

                                for i in 0..4 {
                                    let p = corners[i];

                                    if w > 0 {
                                        if p.y >= p1.y {
                                            if p.x < p1.x {
                                                winding[i] = w;
                                            }
                                            continue;
                                        } else if p.y <= p0.y {
                                            if p.x < p0.x {
                                                winding[i] = w;
                                            }
                                            continue;
                                        }
                                    } else {
                                        if p.y >= p0.y {
                                            if p.x < p0.x {
                                                winding[i] = w;
                                            }
                                            continue;
                                        } else if p.y <= p1.y {
                                            if p.x < p1.x {
                                                winding[i] = w;
                                            }
                                            continue;
                                        }
                                    }

                                    let p0p = p - p0;
                                    let wps = p0p1.cross(p0p) * dy >= 0.0;
                                    if wps {
                                        winding[i] = w;
                                    }
                                }

                                if winding[0] != winding[1]
                                    || winding[2] != winding[3]
                                    || winding[0] != winding[2]
                                    || winding[1] != winding[3]
                                {
                                    new_path.push(*segment);
                                } else if winding[2] == winding[3] && winding[1] != 0 {
                                    if y_min < corners[3].y && corners[3].y < y_max {
                                        backdrop += w;
                                    }
                                }
                            }
                            _ => unimplemented!(),
                        }
                    }
                    if new_path.is_empty() {
                        continue;
                    }
                    new_tape.push(new_path);
                }
                if new_tape.is_empty() {
                    continue;
                }

                for iy in y..y + dx {
                    if iy >= height as usize {
                        break;
                    }

                    for ix in x..x + dx {
                        if ix >= width as usize {
                            break;
                        }

                        let index = iy * width as usize + ix;
                        framebuffer[index] = COLOR_LEVELS[tile.level - 1];
                    }
                }

                if tile.level > 1 {
                    coarse_tiles.push_back(CoarseTile {
                        tx: tile.tx * SUBDIVISION + sx,
                        ty: tile.ty * SUBDIVISION + sy,
                        level: tile.level - 1,
                        tape: new_tape,
                    });
                } else {
                    fine_tiles.push(FineTile {
                        x,
                        y,
                        tape: new_tape,
                    });
                }
            }
        }
    }

    dbg!(fine_tiles.len());

    for tile in fine_tiles {
        let index = tile.y * width as usize + tile.x;

        framebuffer[index] = rgb(0.0, 0.0, 0.0);

        let mut coverage = 0.0;

        for sy in 0..SAMPLES {
            for sx in 0..SAMPLES {
                let p = Point::new(
                    tile.x as f64 + sx as f64 / (SAMPLES + 1) as f64,
                    tile.y as f64 + sy as f64 / (SAMPLES + 1) as f64,
                );

                for path in &tile.tape {
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
