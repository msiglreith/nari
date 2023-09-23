use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use softbuffer::GraphicsContext;
use std::ops::{Index, IndexMut, Range};

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
struct vec2 {
    x: f32,
    y: f32,
}

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
struct rgbaf32 {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl rgbaf32 {
    const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
}

struct Image {
    memory: Box<[rgbaf32]>,
    width: u32,
    height: u32,
    row_pitch: usize,
}

impl Image {
    fn new(width: u32, height: u32) -> Self {
        Self {
            memory: vec![
                rgbaf32 {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                (width * height) as usize
            ]
            .into(),
            width,
            height,
            row_pitch: width as usize,
        }
    }
}

type FragmentIdx = (u32, u32);

impl Index<FragmentIdx> for Image {
    type Output = rgbaf32;
    fn index(&self, (x, y): FragmentIdx) -> &Self::Output {
        let idx = x as usize + y as usize * self.row_pitch;
        &self.memory[idx]
    }
}

impl IndexMut<FragmentIdx> for Image {
    fn index_mut(&mut self, (x, y): FragmentIdx) -> &mut Self::Output {
        let idx = x as usize + y as usize * self.row_pitch;
        &mut self.memory[idx]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Direction {
    Positive,
    Negative,
}

const SAMPLES: u32 = 8;
const SAMPLE_LOCATIONS: [i32; 16] = [0, 5, 3, 7, 1, 4, 6, 2, 0, 5, 3, 7, 1, 4, 6, 2];
// const SAMPLE_LOCATIONS: [i32; 16] = [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7];

const QUAD_SIZE: u32 = 2;
const QUAD_SIZE_F32: f32 = QUAD_SIZE as f32;

#[derive(Copy, Clone, Debug)]
struct CoverageQuad {
    x: u16,
    y: u16,

    /// Winding of line according to x/y rays
    winding: i32,
    coverage: Coverage,
}

#[derive(Copy, Clone, Debug)]
enum Coverage {
    /// Fill with winding number of topleft corner.
    Fill(u32),

    /// Sample mask for the quad patch.
    /// 8 samples per pixel.
    Mask(u32),
}

/// Number of quads in each dimension.
const TILE_SIZE: u16 = 8;

#[derive(Copy, Clone)]
struct Tile {
    x: u16,
    y: u16,

    quad_start: usize,
    quad_mask: u64,
}

#[derive(Copy, Clone, Debug)]
struct Intersect {
    x: u16,
    y: u16,
    winding: i32,
}

fn traverse_line(frame: FrameParams, p0: vec2, p1: vec2) -> (Vec<CoverageQuad>, Vec<Intersect>) {
    let mut quads = Vec::default();
    let mut intersects = Vec::default();

    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;

    let winding_x = if dy > 0.0 { 1 } else { -1 };
    let winding_y = if dx > 0.0 { -1 } else { 1 };

    let (y0, y1, mut x, ty0, ty1, mut tx, dx) = if p0.y < p1.y {
        let dx = (p1.x - p0.x) / (p1.y - p0.y);

        let py = p0.y * 8.0 - 0.5;
        let y0 = (p0.y * 8.0 - 0.5).ceil();
        let y1 = (p1.y * 8.0 - 0.5).ceil();
        let x = p0.x * 8.0 - 0.5 + (y0 - py) * dx;

        let py = p0.y / 2.0;
        let ty0 = (p0.y / 2.0).ceil();
        let ty1 = (p1.y / 2.0).ceil();
        let tx = p0.x / 2.0 + (y0 - py) * dx;

        (y0, y1, x, ty0, ty1, tx, dx)
    } else {
        let dx = (p0.x - p1.x) / (p0.y - p1.y);

        let py = p1.y * 8.0 - 0.5;
        let y0 = (p1.y * 8.0 + 0.5).floor();
        let y1 = (p0.y * 8.0 + 0.5).floor();
        let x = p1.x * 8.0 - 0.5 + (y0 - py) * dx;

        let py = p1.y / 2.0;
        let ty0 = (p1.y / 2.0 + 1.0).floor();
        let ty1 = (p0.y / 2.0 + 1.0).floor();
        let tx = p1.x / 2.0 + (y0 - py) * dx;

        (y0, y1, x, ty0, ty1, tx, dx)
    };

    let mut mask = 0u32;

    let mut prev_x = x as i32 / 16;
    let mut prev_y = y0 as i32 / 16;

    for y in y0 as i32..y1 as i32 {
        let ty = y / 16;
        let tx = x as i32 / 16;
        let sy = y % 16;
        let sx = x as i32 % 16;

        if sy == 0 && ty >= 0 {
            intersects.push(Intersect {
                x: (tx + 1).max(0) as _,
                y: ty as _,
                winding: winding_x,
            });
        }

        {
            if ty != prev_y {
                dbg!((prev_y, mask));
                quads.push(CoverageQuad {
                    x: prev_x as _,
                    y: prev_y as _,
                    winding: winding_x,
                    coverage: Coverage::Mask(mask),
                });

                mask = 0;
            } else if tx != prev_x {
                dbg!((prev_x, mask));
                quads.push(CoverageQuad {
                    x: prev_x as _,
                    y: prev_y as _,
                    winding: winding_x,
                    coverage: Coverage::Mask(mask),
                });
                mask = 0;

                let y_mask = (!((1 << sy) - 1)) & 0xFFFF;
                quads.push(CoverageQuad {
                    x: prev_x.max(tx) as _,
                    y: prev_y as _,
                    winding: winding_y,
                    coverage: Coverage::Mask(y_mask | (y_mask << 16)),
                });
            }
        }

        let loc = SAMPLE_LOCATIONS[sy as usize];
        if loc >= sx {
            mask |= 1 << sy;
        }
        if loc + 8 >= sx {
            mask |= 1 << (sy + 16);
        }

        x += dx;
        prev_x = tx;
        prev_y = ty;
    }

    if mask != 0 {
        quads.push(CoverageQuad {
            x: prev_x as _,
            y: prev_y as _,
            winding: winding_x,
            coverage: Coverage::Mask(mask),
        });
    }

    dbg!(&quads);

    (quads, intersects)
}

fn draw_rect(target: &mut Image, x: Range<u32>, y: Range<u32>, color: rgbaf32) {
    for iy in y {
        for ix in x.clone() {
            target[(ix, iy)] = color;
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct FrameParams {
    /// Number of quads the framebuffer can fit in x direction.
    width_quads: u16,
    /// Number of quads the framebuffer can fit in y direction.
    height_quads: u16,
}

fn draw(width: u32, height: u32) -> Vec<u32> {
    // clear output buffer
    let mut output = Image::new(width, height);

    let frame_params = FrameParams {
        width_quads: ((width + QUAD_SIZE - 1) / QUAD_SIZE) as u16,
        height_quads: ((height + QUAD_SIZE - 1) / QUAD_SIZE) as u16,
    };

    draw_rect(&mut output, 10..40, 10..20, rgbaf32::WHITE);

    let cx = 101.0;
    let cy = 100.0;
    let size = 10.0;

    let path = [
        [
            vec2 {
                x: cx,
                y: cy - size,
            },
            vec2 {
                x: cx + size,
                y: cy,
            },
        ],
        [
            vec2 {
                x: cx + size,
                y: cy,
            },
            vec2 {
                x: cx,
                y: cy + size,
            },
        ],
        [
            vec2 {
                x: cx,
                y: cy + size,
            },
            vec2 {
                x: cx - size,
                y: cy,
            },
        ],
        [
            vec2 {
                x: cx - size,
                y: cy,
            },
            vec2 {
                x: cx,
                y: cy - size,
            },
        ],
    ];

    let mut quads = Vec::default();
    let mut intersects = Vec::default();
    for [p0, p1] in path {
        let (path_quads, path_intersects) = traverse_line(frame_params, p0, p1);
        quads.extend(path_quads);
        intersects.extend(path_intersects);
    }

    intersects.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));
    dbg!(&intersects);

    // Visualize intersects
    let mut prev_x = intersects[0].x;
    let mut prev_y = intersects[0].y;
    let mut winding = 0i32;
    for intersect in intersects {
        if intersect.y != prev_y {
            winding = intersect.winding;
        } else {
            dbg!((winding, intersect.winding));
            if winding != 0 {
                for x in prev_x..intersect.x {
                    quads.push(CoverageQuad {
                        x: x,
                        y: intersect.y,
                        winding: winding.signum(),
                        coverage: Coverage::Fill(winding.abs() as _),
                    });
                    // for iy in 0..QUAD_SIZE {
                    //     for ix in 0..QUAD_SIZE {
                    //         let idx = (
                    //             x as u32 * QUAD_SIZE + ix,
                    //             intersect.y as u32 * QUAD_SIZE + iy,
                    //         );
                    //         output[idx] = rgbaf32 {
                    //             r: 1.0,
                    //             g: 1.0,
                    //             b: 1.0,
                    //             a: 1.0,
                    //         };
                    //     }
                    // }
                }
            }
            winding += intersect.winding;
        }

        prev_x = intersect.x;
        prev_y = intersect.y;
    }

    // Sort tile (path/y/x) to generate y-spans in the next step
    quads.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));

    let mut prev_x = quads[0].x;
    let mut prev_y = quads[0].y;
    let mut winding = [0i32; 32];

    dbg!(&quads);

    // Visualize local coverage quads
    for quad in quads {
        if prev_x != quad.x || prev_y != quad.y {
            for iy in 0..QUAD_SIZE {
                for ix in 0..QUAD_SIZE {
                    let idx = (
                        prev_x as u32 * QUAD_SIZE + ix,
                        prev_y as u32 * QUAD_SIZE + iy,
                    );
                    let s = iy + ix * QUAD_SIZE;

                    // Split sample mask into pixel.
                    let mut samples = 0;
                    for i in (s * 8)..((s + 1) * 8) {
                        if winding[i as usize] != 0 {
                            samples += 1;
                        }
                    }
                    let coverage = samples as f32 / 7.0;

                    output[idx] = rgbaf32 {
                        r: coverage,
                        g: coverage,
                        b: coverage,
                        a: 1.0,
                    };
                }
            }

            prev_x = quad.x;
            prev_y = quad.y;
            winding = [0; 32];
        }

        match quad.coverage {
            Coverage::Fill(num) => {
                for s in 0..32 {
                    winding[s] += num as i32 * quad.winding;
                }
            }
            Coverage::Mask(mask) => {
                for s in 0..32 {
                    if mask & (1 << s) != 0 {
                        winding[s] += quad.winding;
                    }
                }
            }
        }
    }

    // resolve output to framebuffer
    let mut framebuffer = vec![0; (width * height) as usize];
    for i in 0..output.memory.len() {
        let c = output.memory[i];
        let r = (255.0 * c.r.clamp(0.0, 1.0)) as u32;
        let g = (255.0 * c.g.clamp(0.0, 1.0)) as u32;
        let b = (255.0 * c.b.clamp(0.0, 1.0)) as u32;

        framebuffer[i] = b | (g << 8) | (r << 16);
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
                let buffer = draw(width as _, height as _);
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
