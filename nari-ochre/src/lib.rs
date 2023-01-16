use std::ops::Range;
use zeno::{PathBuilder, Point};

pub const TILE_SIZE: usize = 8;
const TOLERANCE: f32 = 0.1;

#[derive(Clone, Copy, Debug)]
struct Increment {
    x: i16,
    y: i16,
    area: f32,
    height: f32,
}

#[derive(Clone, Copy)]
struct TileIncrement {
    tile_x: i16,
    tile_y: i16,
    sign: i8,
}

#[derive(Clone, Copy)]
struct Bin {
    tile_x: i16,
    tile_y: i16,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct TileRange {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
}

pub type Tile<T> = [[T; TILE_SIZE]; TILE_SIZE];

pub trait Encoder {
    fn solid(&mut self, y: i16, x: Range<i16>);
    fn mask(&mut self, y: i16, x: i16, mask: &Tile<u8>);
}

#[derive(Default)]
pub struct Rasterizer {
    start: Point,
    cur: Point,

    row_prev: i16,

    increments: Vec<Increment>,
    tile_increments: Vec<TileIncrement>,
    bins: Vec<Bin>,
}

impl Rasterizer {
    pub fn begin(&mut self) {
        self.increments.clear();
        self.tile_increments.clear();
    }

    pub fn range(&self) -> TileRange {
        let mut range = TileRange {
            top: i16::MAX,
            bottom: i16::MIN,
            left: i16::MAX,
            right: i16::MIN,
        };
        for increment in &self.increments {
            let tx = increment.x.wrapping_div_euclid(TILE_SIZE as i16);
            let ty = increment.y.wrapping_div_euclid(TILE_SIZE as i16);

            range.left = range.left.min(tx);
            range.right = range.right.max(tx + 1);

            range.top = range.top.min(ty);
            range.bottom = range.bottom.max(ty + 1);
        }
        range
    }

    pub fn end(&mut self, encoder: &mut impl Encoder) {
        self.close();

        self.bins.clear();
        let mut bin = Bin {
            tile_x: 0,
            tile_y: 0,
            start: 0,
            end: 0,
        };
        if let Some(first) = self.increments.first() {
            bin.tile_x = (first.x as i16).wrapping_div_euclid(TILE_SIZE as i16);
            bin.tile_y = (first.y as i16).wrapping_div_euclid(TILE_SIZE as i16);
        }

        for (i, increment) in self.increments.iter().enumerate() {
            let tile_x = increment.x.wrapping_div_euclid(TILE_SIZE as i16);
            let tile_y = increment.y.wrapping_div_euclid(TILE_SIZE as i16);
            if tile_x != bin.tile_x || tile_y != bin.tile_y {
                self.bins.push(bin);
                bin = Bin {
                    tile_x,
                    tile_y,
                    start: i,
                    end: i,
                };
            }
            bin.end += 1;
        }
        self.bins.push(bin);

        self.bins
            .sort_unstable_by_key(|bin| (bin.tile_y, bin.tile_x));
        self.tile_increments
            .sort_unstable_by_key(|tile_inc| (tile_inc.tile_y, tile_inc.tile_x));

        let mut areas = [0.0; TILE_SIZE * TILE_SIZE];
        let mut heights = [0.0; TILE_SIZE * TILE_SIZE];
        let mut prev = [0.0; TILE_SIZE];
        let mut next = [0.0; TILE_SIZE];

        let mut tile_increments_i = 0;
        let mut winding = 0;

        for i in 0..self.bins.len() {
            let bin = self.bins[i];
            for increment in &self.increments[bin.start..bin.end] {
                let x = (increment.x as usize).wrapping_rem_euclid(TILE_SIZE);
                let y = (increment.y as usize).wrapping_rem_euclid(TILE_SIZE);
                areas[(y * TILE_SIZE + x) as usize] += increment.area;
                heights[(y * TILE_SIZE + x) as usize] += increment.height;
            }

            if i + 1 == self.bins.len()
                || self.bins[i + 1].tile_x != bin.tile_x
                || self.bins[i + 1].tile_y != bin.tile_y
            {
                let mut tile = [[0; TILE_SIZE]; TILE_SIZE];
                for y in 0..TILE_SIZE {
                    let mut accum = prev[y];
                    for x in 0..TILE_SIZE {
                        tile[y][x] =
                            ((accum + areas[y * TILE_SIZE + x]).abs() * 256.0).min(255.0) as u8;
                        accum += heights[y * TILE_SIZE + x];
                    }
                    next[y] = accum;
                }

                encoder.mask(bin.tile_y, bin.tile_x, &tile);

                areas = [0.0; TILE_SIZE * TILE_SIZE];
                heights = [0.0; TILE_SIZE * TILE_SIZE];
                if i + 1 < self.bins.len() && self.bins[i + 1].tile_y == bin.tile_y {
                    prev = next;
                } else {
                    prev = [0.0; TILE_SIZE];
                }
                next = [0.0; TILE_SIZE];

                if i + 1 < self.bins.len()
                    && self.bins[i + 1].tile_y == bin.tile_y
                    && self.bins[i + 1].tile_x > bin.tile_x + 1
                {
                    while tile_increments_i < self.tile_increments.len() {
                        let tile_increment = self.tile_increments[tile_increments_i];
                        if (tile_increment.tile_y, tile_increment.tile_x) > (bin.tile_y, bin.tile_x)
                        {
                            break;
                        }
                        winding += tile_increment.sign as isize;
                        tile_increments_i += 1;
                    }
                    if winding != 0 {
                        let width = self.bins[i + 1].tile_x - bin.tile_x - 1;
                        let x0 = bin.tile_x + 1;
                        let x1 = x0 + width;
                        encoder.solid(bin.tile_y, x0..x1);
                    }
                }
            }
        }
    }
}

fn lerp(p0: Point, p1: Point, t: f32) -> Point {
    p0 * (1.0 - t) + p1 * t
}

impl PathBuilder for Rasterizer {
    fn current_point(&self) -> Point {
        self.start
    }

    fn move_to(&mut self, to: impl Into<Point>) -> &mut Self {
        let to: Point = to.into();

        self.row_prev = (to.y.floor() as i16).wrapping_div_euclid(TILE_SIZE as i16);

        self.start = to;
        self.cur = to;
        self
    }

    fn line_to(&mut self, to: impl Into<Point>) -> &mut Self {
        let to: Point = to.into();

        if self.cur == to {
            return self;
        }

        let x_dir = (to.x - self.cur.x).signum() as i16;
        let y_dir = (to.y - self.cur.y).signum() as i16;
        let dtdx = 1.0 / (to.x - self.cur.x);
        let dtdy = 1.0 / (to.y - self.cur.y);
        let mut x = self.cur.x.floor() as i16;
        let mut y = self.cur.y.floor() as i16;
        let mut row_t0: f32 = 0.0;
        let mut col_t0: f32 = 0.0;
        let mut row_t1 = if self.cur.y == to.y {
            std::f32::INFINITY
        } else {
            let next_y = if to.y > self.cur.y {
                (y + 1) as f32
            } else {
                y as f32
            };
            (dtdy * (next_y - self.cur.y)).min(1.0)
        };
        let mut col_t1 = if self.cur.x == to.x {
            std::f32::INFINITY
        } else {
            let next_x = if to.x > self.cur.x {
                (x + 1) as f32
            } else {
                x as f32
            };
            (dtdx * (next_x - self.cur.x)).min(1.0)
        };
        let x_step = dtdx.abs();
        let y_step = dtdy.abs();

        loop {
            let t0 = if row_t0 > col_t0 { row_t0 } else { col_t0 };
            let t1 = if row_t1 < col_t1 { row_t1 } else { col_t1 };
            let p0 = lerp(self.cur, to, t0);
            let p1 = lerp(self.cur, to, t1);
            let height = p1.y - p0.y;
            let right = (x + 1) as f32;
            let area = 0.5 * height * ((right - p0.x) + (right - p1.x));

            self.increments.push(Increment { x, y, area, height });

            if row_t1 < col_t1 {
                row_t0 = row_t1;
                row_t1 = row_t1 + y_step;
                if row_t1 > 1.0 {
                    row_t1 = 1.0;
                }
                y += y_dir;
            } else {
                col_t0 = col_t1;
                col_t1 = col_t1 + x_step;
                if col_t1 > 1.0 {
                    col_t1 = 1.0;
                }
                x += x_dir;
            }

            if row_t0 == 1.0 || col_t0 == 1.0 {
                x = to.x.floor() as i16;
                y = to.y.floor() as i16;
            }

            let tile_y = y.wrapping_div_euclid(TILE_SIZE as i16);
            if tile_y != self.row_prev {
                self.tile_increments.push(TileIncrement {
                    tile_x: x.wrapping_div_euclid(TILE_SIZE as i16),
                    tile_y: self.row_prev.min(tile_y),
                    sign: (tile_y - self.row_prev) as i8,
                });
                self.row_prev = tile_y;
            }

            if row_t0 == 1.0 || col_t0 == 1.0 {
                break;
            }
        }

        self.cur = to;
        self
    }

    fn quad_to(&mut self, control1: impl Into<Point>, to: impl Into<Point>) -> &mut Self {
        let p0 = self.cur;
        let p1: Point = to.into();
        let control1: Point = control1.into();

        let dt = ((4.0 * TOLERANCE) / (p0 - control1 * 2.0 + p1).length()).sqrt();

        let mut t = 0.0;
        while t < 1.0 {
            t = (t + dt).min(1.0);

            let p01 = lerp(p0, control1, t);
            let p12 = lerp(control1, p1, t);
            let p = lerp(p01, p12, t);

            self.line_to(p);
        }

        self
    }

    fn curve_to(
        &mut self,
        control1: impl Into<Point>,
        control2: impl Into<Point>,
        to: impl Into<Point>,
    ) -> &mut Self {
        let p0 = self.cur;
        let p1: Point = to.into();
        let control1: Point = control1.into();
        let control2: Point = control2.into();

        let a = p0 * -1.0 + control1 * 3.0 - control2 * 3.0 + p1;
        let b = (p0 - control1 * 2.0 + control2) * 3.0;
        let conc = b.length().max((a + b).length());
        let dt = ((8.0f32.sqrt() * TOLERANCE) / conc).sqrt();

        let mut t = 0.0;
        while t < 1.0 {
            t = (t + dt).min(1.0);
            let p01 = lerp(p0, control1, t);
            let p12 = lerp(control1, control2, t);
            let p23 = lerp(control2, p1, t);
            let p012 = lerp(p01, p12, t);
            let p123 = lerp(p12, p23, t);
            let p = lerp(p012, p123, t);

            self.line_to(p);
        }

        self
    }

    fn close(&mut self) -> &mut Self {
        if self.start != self.cur {
            self.line_to(self.start);
        }
        self
    }
}
