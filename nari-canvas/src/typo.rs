//! Typographics

use crate::{
    fxp::fxp6,
    layout::{Align, Rect},
    Color, RasterTiles,
};
use std::collections::HashMap;
use std::ops::Range;

pub type Font = usize;
pub type FontSize = u32;

#[derive(Copy, Clone, Debug)]
pub struct Pen {
    pub x: i32,
    pub y: i32,

    pub align_x: Align,

    pub color: Color,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            color: [0.0, 0.0, 0.0, 1.0],
            align_x: Align::Positive,
        }
    }
}

pub type GlyphId = u32;

#[derive(Debug, Clone, Copy)]
pub struct FontProperties {
    pub ascent: i32,
    pub descent: i32,
    pub height: i32,
}

impl FontProperties {
    pub fn alignment_height(&self) -> i32 {
        self.ascent + self.descent
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub id: GlyphId,
    pub offset: fxp6, // subpixel x offset
}
pub(crate) type GlyphCache = HashMap<(FontSize, GlyphKey), RasterTiles>;

#[derive(Debug, Clone, Copy)]
pub struct FontScaled {
    pub font: Font,
    pub size: FontSize,
    pub properties: FontProperties,
}

pub struct FontBrush<'a> {
    pub font: &'a mut FontScaled,
}

pub struct TextRun {
    pub(crate) font: FontScaled,
    pub clusters: Vec<TextRunGraphemeCluster>,
    pub(crate) width: f32,
}

pub struct Caret {
    pub cluster: usize,
}

impl TextRun {
    pub fn width(&self) -> i32 {
        self.width.round() as i32
    }

    pub fn offset_x(&self, align: Align) -> i32 {
        match align {
            Align::Negative => -self.width(),
            Align::Center => -(self.width / 2.0).round() as i32,
            Align::Positive => 0,
        }
    }

    pub fn bounds(&self, pen: Pen) -> Rect {
        let ox = self.offset_x(pen.align_x);

        let x0 = pen.x + ox;
        let x1 = x0 + self.width();
        let y0 = pen.y - self.font.properties.ascent;
        let y1 = pen.y - self.font.properties.descent;

        Rect { x0, x1, y0, y1 }
    }

    pub fn hittest(&self, pen: Pen, x: i32, y: i32) -> Option<Caret> {
        const HITTEST_MARGIN_PX: i32 = 2; // percentage rather of the current glyph?

        let bounds = self.bounds(pen);
        if !bounds.hittest(x, y) {
            return None;
        }

        let relativ_x = x - bounds.x0 + HITTEST_MARGIN_PX;
        for (i, cluster) in self.clusters.iter().enumerate() {
            if cluster.advances.contains(&relativ_x) {
                return Some(Caret { cluster: i });
            }
        }

        Some(Caret {
            cluster: self.clusters.len(),
        })
    }

    pub fn cluster_advance(&self, byte_pos: usize) -> i32 {
        let mut advance = 0;
        for cluster in &self.clusters {
            if byte_pos < cluster.byte_pos {
                return advance;
            }
            advance = cluster.advances.start;
            if byte_pos == cluster.byte_pos {
                return advance;
            }
        }
        self.width()
    }
}

#[derive(Debug)]
pub struct TextRunGraphemeCluster {
    pub byte_pos: usize,
    pub(crate) glyphs: Vec<TextRunGlyph>,
    pub advances: Range<i32>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct TextRunGlyph {
    pub id: GlyphId,
    pub offset: fxp6,
}
