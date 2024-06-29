//! Typographics

use crate::{fxp::fxp6, Align};
use parley::{fontique::Attributes, Layout};
use std::collections::HashMap;
use std::ops::Range;
use vello::kurbo::{BezPath, Point, Rect, Shape};

pub use parley::layout::Cursor;

pub type Font = usize;
pub type FontSize = u32;
pub type GlyphId = u32;

#[derive(Debug, Clone, Copy)]
pub struct FontProperties {
    pub ascent: f64,
    pub descent: f64,
    pub height: f64,
}

impl FontProperties {
    pub fn alignment_height(&self) -> f64 {
        self.ascent + self.descent
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub id: GlyphId,
}
pub type GlyphCache = HashMap<(FontSize, GlyphKey), BezPath>;

#[derive(Debug, Clone, Copy)]
pub struct FontScaled {
    pub font: Font,
    pub size: FontSize,
    pub attributes: Attributes,
    pub properties: FontProperties,
}

pub struct FontBrush<'a> {
    pub font: &'a mut FontScaled,
}

pub struct TextRun {
    pub font: FontScaled,
    pub layout: Layout<[u8; 4]>,
}

impl TextRun {
    pub fn width(&self) -> f64 {
        self.layout.width() as _
    }

    pub fn offset_x(&self, align: Align) -> f64 {
        match align {
            Align::Negative => -self.width(),
            Align::Center => -self.width() / 2.0,
            Align::Positive => 0.0,
        }
    }

    pub fn bounds(&self) -> Rect {
        let x0 = 0.0;
        let x1 = self.layout.width() as _;
        let y0 = 0.0;
        let y1 = self.layout.height() as _;

        Rect { x0, x1, y0, y1 }
    }

    pub fn hittest(&self, p: Point) -> Option<Cursor> {
        let bounds = self.bounds();
        if bounds.winding(p) <= 0 {
            return None;
        }

        let relative_x = p.x - bounds.x0;
        let relative_y = p.y - bounds.y0;
        Some(Cursor::from_point(
            &self.layout,
            relative_x as f32,
            relative_y as f32,
        ))
    }
}

#[derive(Debug)]
pub struct TextRunGraphemeCluster {
    pub byte_pos: usize,
    pub glyphs: Vec<TextRunGlyph>,
    pub advances: Range<f64>,
}

#[derive(Copy, Clone, Debug)]
pub struct TextRunGlyph {
    pub id: GlyphId,
    pub offset: fxp6,
}
