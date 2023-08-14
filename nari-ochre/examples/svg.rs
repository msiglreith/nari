use nari_ochre::{Encoder, Rasterizer, Tile, TileRange, TILE_SIZE};
use std::ops::Range;
use zeno::{apply, Cap, Command, Fill, Join, Mask, PathData, Point, Stroke, Style, Transform};

struct ImageEncoder {
    range: TileRange,
    image: Vec<u8>,
}

impl ImageEncoder {
    pub fn new(range: TileRange) -> Self {
        let nx = (range.right - range.left) as usize;
        let ny = (range.bottom - range.top) as usize;
        ImageEncoder {
            range,
            image: vec![5; nx * TILE_SIZE * ny * TILE_SIZE],
        }
    }
}

impl Encoder for ImageEncoder {
    fn solid(&mut self, y: i16, x: Range<i16>) {
        let ty = (y - self.range.top) as usize;
        let nx = (self.range.right - self.range.left) as usize * TILE_SIZE;

        for dx in x {
            let tx = (dx - self.range.left) as usize;

            let ox = tx * TILE_SIZE;
            let oy = ty * TILE_SIZE;

            for py in 0..TILE_SIZE {
                for px in 0..TILE_SIZE {
                    self.image[(ox + px) + (oy + py) * nx] = 255;
                }
            }
        }
    }

    fn mask(&mut self, y: i16, x: i16, mask: &Tile<u8>) {
        let nx = (self.range.right - self.range.left) as usize * TILE_SIZE;

        let tx = (x - self.range.left) as usize;
        let ty = (y - self.range.top) as usize;

        let ox = tx * TILE_SIZE;
        let oy = ty * TILE_SIZE;

        for py in 0..TILE_SIZE {
            let row = &mut self.image[(oy + py) * nx..];
            for px in 0..TILE_SIZE {
                row[(ox + px)] = mask[py][px];
            }
        }
    }
}

pub struct Sink;
impl Encoder for Sink {
    fn solid(&mut self, _y: i16, _x: Range<i16>) {}

    fn mask(&mut self, _y: i16, _x: i16, _mask: &Tile<u8>) {}
}

struct PathCommand<'a> {
    style: Style<'a>,
    transform: Option<Transform>,
}

fn encode_node<F>(node: &usvg::Node, encode: &mut F)
where
    F: FnMut(PathCommand, &[Command]),
{
    match *node.borrow() {
        usvg::NodeKind::Path(ref p) => {
            let mut path = Vec::<Command>::new();
            for segment in p.data.0.iter() {
                dbg!(segment);
                match *segment {
                    usvg::PathSegment::MoveTo { x, y } => {
                        path.push(Command::MoveTo(Point::new(x as f32, y as f32)));
                    }
                    usvg::PathSegment::LineTo { x, y } => {
                        path.push(Command::LineTo(Point::new(x as f32, y as f32)));
                    }
                    usvg::PathSegment::CurveTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        path.push(Command::CurveTo(
                            Point::new(x1 as f32, y1 as f32),
                            Point::new(x2 as f32, y2 as f32),
                            Point::new(x as f32, y as f32),
                        ));
                    }
                    usvg::PathSegment::ClosePath => {
                        path.push(Command::Close);
                    }
                }
            }

            if let Some(_) = p.fill {
                (encode)(
                    PathCommand {
                        style: Style::Fill(Fill::EvenOdd),
                        transform: None,
                    },
                    &path,
                );
            }

            if let Some(ref s) = p.stroke {
                (encode)(
                    PathCommand {
                        style: Style::Stroke(Stroke {
                            width: s.width.value() as _,
                            join: Join::Round,
                            miter_limit: s.miterlimit.value() as _,
                            start_cap: Cap::Round,
                            end_cap: Cap::Round,
                            dashes: &[],
                            offset: 0.0,
                            scale: true,
                        }),
                        transform: None,
                    },
                    &path,
                );
            }
        }
        _ => {}
    }

    for child in node.children() {
        encode_node(&child, encode);
    }
}

fn main() {
    let args = &std::env::args().collect::<Vec<_>>();
    let file_path = &args[1];
    let svg_data = std::fs::read_to_string(file_path).unwrap();
    let svg = usvg::Tree::from_str(&svg_data, &usvg::Options::default().to_ref()).unwrap();

    let mut rasterizer = Rasterizer::default();

    let mut i = 0;
    encode_node(&svg.root(), &mut |cmd, path| {
        let mut path_flat = Vec::<Command>::new();
        apply(path, cmd.style, cmd.transform, &mut path_flat);

        // zeno reference
        let (mask, place) = Mask::new(&path).render();
        image::save_buffer(
            &format!("{}.zeno_{}.png", file_path, i),
            &mask,
            place.width,
            place.height,
            image::ColorType::L8,
        )
        .unwrap();

        // nari
        rasterizer.begin();
        (&path_flat).copy_to(&mut rasterizer);
        let range = rasterizer.range();
        let mut encoder = ImageEncoder::new(range);
        rasterizer.end(&mut encoder);

        image::save_buffer(
            &format!("{}.nari_{}.png", file_path, i),
            &encoder.image,
            (range.right - range.left) as u32 * TILE_SIZE as u32,
            (range.bottom - range.top) as u32 * TILE_SIZE as u32,
            image::ColorType::L8,
        )
        .unwrap();

        i += 1;
    });
}
