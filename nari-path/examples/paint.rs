use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use softbuffer::GraphicsContext;
use std::ops::{Index, IndexMut};

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

fn traverse_line(target: &mut Image, p0: vec2, p1: vec2) {
    const TILE_SIZE: f32 = 1.0;

    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;

    let mut tile_x = (p0.x / TILE_SIZE).floor();
    let mut tile_y = (p0.y / TILE_SIZE).floor();

    let (dtile_x, mut dt_x, ddt_x) = if dx > 0.0 {
        (
            1.0,
            (tile_x * TILE_SIZE + TILE_SIZE - p0.x) / dx,
            TILE_SIZE / dx,
        )
    } else if dx < 0.0 {
        (-1.0, (tile_x * TILE_SIZE - p0.x) / dx, -TILE_SIZE / dx)
    } else {
        (0.0, 1.0, 1.0)
    };
    let (dtile_y, mut dt_y, ddt_y) = if dy > 0.0 {
        (
            1.0,
            (tile_y * TILE_SIZE + TILE_SIZE - p0.y) / dy,
            TILE_SIZE / dy,
        )
    } else if dy < 0.0 {
        (-1.0, (tile_y * TILE_SIZE - p0.y) / dy, -TILE_SIZE / dy)
    } else {
        (0.0, 1.0, 1.0)
    };

    let length = (dx * dx + dy * dy).sqrt(); // todo: length == 0

    let mut t = 0.0;
    while t < 1.0 {
        if tile_x >= 0.0 && tile_y >= 0.0 {
            target[(tile_x as u32, tile_y as u32)] = rgbaf32::WHITE;
        }

        if dt_x < dt_y {
            t += dt_x;
            tile_x += dtile_x;
            dt_y -= dt_x;
            dt_x = ddt_x;
        } else {
            t += dt_y;
            tile_y += dtile_y;
            dt_x -= dt_y;
            dt_y = ddt_y;
        }
    }
}

fn draw(width: u32, height: u32) -> Vec<u32> {
    // clear output buffer
    let mut output = Image::new(width, height);

    for y in 10..20 {
        for x in 10..40 {
            output[(x, y)] = rgbaf32::WHITE;
        }
    }

    traverse_line(
        &mut output,
        vec2 { x: 100.0, y: 30.0 },
        vec2 { x: 300.0, y: 60.0 },
    );

    traverse_line(
        &mut output,
        vec2 { x: 500.0, y: 60.0 },
        vec2 { x: 300.0, y: 30.0 },
    );

    traverse_line(
        &mut output,
        vec2 { x: 100.0, y: 100.0 },
        vec2 { x: 300.0, y: 70.0 },
    );

    traverse_line(
        &mut output,
        vec2 { x: 500.0, y: 70.0 },
        vec2 { x: 300.0, y: 100.0 },
    );

    traverse_line(
        &mut output,
        vec2 { x: 300.0, y: 120.0 },
        vec2 { x: 500.0, y: 120.0 },
    );

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
