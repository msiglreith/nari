use crate::{fxp::fxp6, RasterTiles, Rect};
use std::collections::HashMap;
use zeno::{Command, Point};

pub struct Squircle {
    pub rect: Rect,
    pub radius: u32,
    pub smoothing: f32,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SquircleKey {
    pub radius: u32,
    pub smoothing: fxp6,
}
pub(crate) type SquircleCache = HashMap<SquircleKey, RasterTiles>;

// Based on https://www.figma.com/blog/desperately-seeking-squircles/
// `kurbo-smooth` for general case, here only for rectangle
pub fn corner(length: f32, smooth: f32) -> Vec<Command> {
    let radius = length / (1.0 + smooth);

    let phi = std::f32::consts::FRAC_PI_4 * smooth;
    let phi2 = std::f32::consts::FRAC_PI_2 * (1.0 - smooth);
    let vc = (0.5 * phi).tan();
    let ab = (vc + smooth) * radius;

    let (s, c) = phi.sin_cos();

    // https://pomax.github.io/bezierinfo/#circles_cubic
    let k = 4.0 / 3.0 * (phi2 / 4.0).tan();

    vec![
        Command::MoveTo(Point::ZERO),
        Command::LineTo(Point { x: length, y: 0.0 }),
        Command::CurveTo(
            Point {
                x: length,
                y: 2.0 / 3.0 * ab,
            },
            Point { x: length, y: ab },
            Point {
                x: radius * (smooth + c),
                y: radius * (smooth + s),
            },
        ),
        // arc approximation
        Command::CurveTo(
            Point {
                x: radius * (smooth + c - s * k),
                y: radius * (smooth + s + c * k),
            },
            Point {
                x: radius * (smooth + s + c * k),
                y: radius * (smooth + c - s * k),
            },
            Point {
                x: radius * (smooth + s),
                y: radius * (smooth + c),
            },
        ),
        Command::CurveTo(
            Point { x: ab, y: length },
            Point {
                x: 2.0 / 3.0 * ab,
                y: length,
            },
            Point { x: 0.0, y: length },
        ),
        Command::Close,
    ]
}
