//! Typographics

use crate::{fxp::fxp6, Align};
use std::collections::HashMap;
use std::ops::Range;
use vello::kurbo::{BezPath, Point, Rect, Shape};

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
    pub offset: fxp6, // subpixel x offset
}
pub type GlyphCache = HashMap<(FontSize, GlyphKey), BezPath>;

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
    pub font: FontScaled,
    pub clusters: Vec<TextRunGraphemeCluster>,
    pub width: f64,
}

pub struct Caret {
    pub cluster: usize,
}

impl TextRun {
    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn offset_x(&self, align: Align) -> f64 {
        match align {
            Align::Negative => -self.width(),
            Align::Center => -self.width / 2.0,
            Align::Positive => 0.0,
        }
    }

    pub fn bounds(&self) -> Rect {
        let x0 = 0.0;
        let x1 = self.width();
        let y0 = -self.font.properties.ascent;
        let y1 = -self.font.properties.descent;

        Rect { x0, x1, y0, y1 }
    }

    pub fn hittest(&self, p: Point) -> Option<Caret> {
        let bounds = self.bounds();
        if bounds.winding(p) <= 0 {
            return None;
        }

        let relativ_x = p.x - bounds.x0;
        for (i, cluster) in self.clusters.iter().enumerate() {
            if cluster.advances.contains(&relativ_x) {
                let idx = if relativ_x
                    <= ((cluster.advances.start + cluster.advances.end) / 2.0).floor()
                {
                    i
                } else {
                    i + 1
                };
                return Some(Caret { cluster: idx });
            }
        }

        Some(Caret {
            cluster: self.clusters.len(),
        })
    }

    pub fn cluster_advance(&self, byte_pos: usize) -> f64 {
        let mut advance = 0.0;
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
    pub glyphs: Vec<TextRunGlyph>,
    pub advances: Range<f64>,
}

#[derive(Copy, Clone, Debug)]
pub struct TextRunGlyph {
    pub id: GlyphId,
    pub offset: fxp6,
}
