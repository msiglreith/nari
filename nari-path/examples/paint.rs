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

#[derive(Copy, Clone, PartialEq, Eq)]
enum Direction {
    Positive,
    Negative,
}

const QUAD_SIZE: u32 = 2;
const QUAD_SIZE_F32: f32 = QUAD_SIZE as f32;

#[derive(Copy, Clone)]
struct CoverageQuad {
    x: u16,
    y: u16,

    /// Line direction, important for raycasting
    dir: Direction,

    /// Sample mask for the quad patch.
    /// 8 samples per pixel.
    samples: u32,
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

fn traverse_line(frame: FrameParams, p0: vec2, p1: vec2) -> Vec<CoverageQuad> {
    let mut quads = Vec::default();

    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;

    let inv_dx = if dx != 0.0 {
        dx.recip()
    } else {
        std::f32::INFINITY
    };
    let inv_dy = if dx != 0.0 {
        dy.recip()
    } else {
        std::f32::INFINITY
    };

    let dir = if dy > 0.0 {
        Direction::Positive
    } else {
        Direction::Negative
    };

    let mut quad_x = (p0.x / QUAD_SIZE_F32).floor();
    let mut quad_y = (p0.y / QUAD_SIZE_F32).floor();

    let (dquad_x, mut dt_x) = if inv_dx > 0.0 {
        (
            1.0,
            (quad_x * QUAD_SIZE_F32 + QUAD_SIZE_F32 - p0.x) * inv_dx,
        )
    } else {
        (-1.0, (quad_x * QUAD_SIZE_F32 - p0.x) * inv_dx)
    };
    let (dquad_y, mut dt_y) = if inv_dy > 0.0 {
        (
            1.0,
            (quad_y * QUAD_SIZE_F32 + QUAD_SIZE_F32 - p0.y) * inv_dy,
        )
    } else {
        (-1.0, (quad_y * QUAD_SIZE_F32 - p0.y) * inv_dy)
    };

    let ddt_x = dquad_x * QUAD_SIZE_F32 * inv_dx;
    let ddt_y = dquad_y * QUAD_SIZE_F32 * inv_dy;

    let mut t = 0.0;
    while t < 1.0 {
        if quad_x >= 0.0
            && quad_y >= 0.0
            && quad_x < frame.width_quads as f32
            && quad_y < frame.height_quads as f32
        {
            quads.push(CoverageQuad {
                x: quad_x as _,
                y: quad_y as _,
                dir,
                samples: !0,
            });
        }

        if dt_x < dt_y {
            t += dt_x;
            quad_x += dquad_x;
            dt_y -= dt_x;
            dt_x = ddt_x;
        } else {
            t += dt_y;
            quad_y += dquad_y;
            dt_x -= dt_y;
            dt_y = ddt_y;
        }
    }

    quads
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

    let cx = 100.0;
    let cy = 100.0;
    let size = 50.0;

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
    for [p0, p1] in path {
        quads.extend(traverse_line(frame_params, p0, p1));
    }

    // Build tiles from quad ranges later used
    // for backdrop calculation to fill in spans.
    //
    // Should be inlined in the quad generation later on
    // to minimize the amount of pipeline steps.
    let mut tiles = Vec::default();

    let mut tile_dir = quads[0].dir;
    let mut tile_x = quads[0].x / TILE_SIZE;
    let mut tile_y = quads[0].y / TILE_SIZE;
    let mut tile_mask = 0;
    let mut tile_start = 0;
    for (i, quad) in quads.iter().enumerate() {
        let x = quad.x / TILE_SIZE;
        let y = quad.y / TILE_SIZE;

        let qx = quad.x % TILE_SIZE;
        let qy = quad.y % TILE_SIZE;
        let quad_mask = qy * TILE_SIZE + qx;

        if x != tile_x || y != tile_y || quad.dir != tile_dir {
            tiles.push(Tile {
                x: tile_x,
                y: tile_y,
                quad_start: tile_start,
                quad_mask: tile_mask,
            });

            tile_x = x;
            tile_y = y;
            tile_mask = 1u64 << quad_mask;
            tile_dir = quad.dir;
            tile_start = i as _;
        } else {
            tile_mask |= 1u64 << quad_mask;
        }
    }

    if tile_mask != 0 {
        tiles.push(Tile {
            x: tile_x,
            y: tile_y,
            quad_start: tile_start,
            quad_mask: tile_mask,
        });
    }

    // Sort tile (path/y/x) to generate y-spans in the next step
    tiles.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));

    // Visualize tiles
    const TILE_PIXELS: u32 = TILE_SIZE as u32 * QUAD_SIZE as u32;

    for (i, tile) in tiles.iter().enumerate() {
        let f = i as f32 / tiles.len() as f32;
        draw_rect(
            &mut output,
            (tile.x as u32 * TILE_PIXELS)..(tile.x + 1) as u32 * TILE_PIXELS,
            (tile.y as u32 * TILE_PIXELS)..(tile.y + 1) as u32 * TILE_PIXELS,
            rgbaf32 {
                r: 1.0 - f,
                g: f,
                b: 0.26,
                a: 1.0,
            },
        );
    }

    // Visualize local coverage quads
    for quad in quads {
        for iy in 0..QUAD_SIZE {
            for ix in 0..QUAD_SIZE {
                let idx = (
                    quad.x as u32 * QUAD_SIZE + ix,
                    quad.y as u32 * QUAD_SIZE + iy,
                );
                let s = iy * QUAD_SIZE + ix;

                // Split sample mask into pixel.
                let samples = (quad.samples >> (s * 8)) & 0xFF;
                let coverage = samples.count_ones() as f32 / 7.0;

                output[idx] = rgbaf32 {
                    r: coverage,
                    g: coverage,
                    b: coverage,
                    a: 1.0,
                };
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
