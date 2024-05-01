use crate::{
    fxp::fxp6,
    typo::{
        Font, FontProperties, FontScaled, FontSize, GlyphCache, GlyphId, GlyphKey, TextRun,
        TextRunGlyph, TextRunGraphemeCluster,
    },
};
use skrifa::prelude::*;
use skrifa::MetadataProvider;
use swash::{shape::ShapeContext, CacheKey, FontRef};
use vello::kurbo::{BezPath, Rect};

pub struct Engine {
    shaper: ShapeContext,
    fonts: Vec<FontData>,
}

impl Engine {
    pub fn new() -> Self {
        let lib = Self {
            shaper: ShapeContext::new(),
            fonts: Vec::default(),
        };

        lib
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> Font {
        let font_id = self.fonts.len();

        // swash
        let font_ref = FontRef::from_index(&data, 0).unwrap();

        self.fonts.push(FontData {
            key: font_ref.key,
            offset: font_ref.offset,
            data,
        });
        font_id
    }

    pub fn create_font_scaled(&mut self, font: Font, size: FontSize) -> FontScaled {
        let font_data: &mut FontData = self.font(font);

        let metrics = font_data
            .to_skrifa()
            .metrics(Size::new(size as _), LocationRef::default());

        let properties = FontProperties {
            ascent: fxp6::from_f32(metrics.ascent).f64(),
            descent: fxp6::from_f32(metrics.descent).f64(),
            height: fxp6::from_f32(metrics.ascent - metrics.descent + metrics.leading).f64(),
        };

        FontScaled {
            font,
            size,
            properties,
        }
    }

    fn font(&mut self, id: Font) -> &mut FontData {
        &mut self.fonts[id]
    }

    pub fn glyph_extent(&mut self, font: FontScaled, c: char) -> Rect {
        let font_data = self.font(font.font);
        let glyph_id = font_data.glyph_index(c);

        let font_ref = font_data.to_skrifa();

        let glyph_metrics =
            font_ref.glyph_metrics(Size::new(font.size as _), LocationRef::default());
        let bounds = glyph_metrics
            .bounds(skrifa::GlyphId::new(glyph_id as _))
            .unwrap();

        Rect {
            x0: fxp6::from_f32(bounds.x_min).f64(),
            y0: fxp6::from_f32(bounds.y_min).f64(),
            x1: fxp6::from_f32(bounds.x_max).f64(),
            y1: fxp6::from_f32(bounds.y_max).f64(),
        }
    }

    pub fn layout_text<S: AsRef<str>>(&mut self, font: FontScaled, text: S) -> TextRun {
        let size_px = font.size as f32;
        let font_ref = self.fonts[font.font].to_ref();

        let mut shaper = self.shaper.builder(font_ref).size(size_px).build();

        shaper.add_str(text.as_ref());

        let mut text_run = TextRun {
            font,
            clusters: Vec::default(),
            width: 0.0,
        };

        let mut advance = fxp6::new(0);
        shaper.shape_with(|cluster| {
            let mut cls = TextRunGraphemeCluster {
                byte_pos: cluster.source.start as _,
                glyphs: Vec::default(),
                advances: advance.f64()..0.0,
            };

            for glyph in cluster.glyphs {
                cls.glyphs.push(TextRunGlyph {
                    id: glyph.id as _,
                    offset: advance,
                });
                advance.0 += fxp6::from_f32(glyph.advance).0;
            }
            cls.advances.end = advance.f64();

            text_run.clusters.push(cls);
        });
        text_run.width = advance.f64(); // todo: includes bearing, not tight /:
        text_run
    }

    pub fn build_text_run<S: AsRef<str>>(
        &mut self,
        font: FontScaled,
        text: S,
        glyph_cache: &mut GlyphCache,
    ) -> TextRun {
        let text_run = self.layout_text(font, text);

        let font_ref = self.font(font.font);

        for cluster in &text_run.clusters {
            for glyph in &cluster.glyphs {
                let subpixel_offset = glyph.offset.fract();
                let glyph_key = GlyphKey {
                    id: glyph.id,
                    offset: subpixel_offset,
                };

                glyph_cache
                    .entry((font.size, glyph_key))
                    .or_insert_with(|| font_ref.outline(font.size, glyph.id, subpixel_offset.0));
            }
        }

        text_run
    }

    pub fn build_glyph(
        &mut self,
        font: FontScaled,
        c: char,
        glyph_cache: &mut GlyphCache,
    ) -> TextRunGlyph {
        let font_ref = self.font(font.font);

        let glyph_key = GlyphKey {
            id: font_ref.glyph_index(c),
            offset: fxp6::new(0),
        };

        glyph_cache
            .entry((font.size, glyph_key))
            .or_insert_with(|| font_ref.outline(font.size, glyph_key.id, glyph_key.offset.0));

        TextRunGlyph {
            id: glyph_key.id,
            offset: glyph_key.offset,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FtFontProperties {
    pub ascent: fxp6,
    pub descent: fxp6,
    pub height: fxp6,
}

struct FontData {
    data: Vec<u8>,

    // swash
    offset: u32,
    key: CacheKey,
}

struct BezPen(BezPath);

impl skrifa::outline::OutlinePen for BezPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, y as f64));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, y as f64));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0
            .quad_to((cx0 as f64, cy0 as f64), (x as f64, y as f64));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (cx0 as f64, cy0 as f64),
            (cx1 as f64, cy1 as f64),
            (x as f64, y as f64),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

impl FontData {
    // swash
    fn to_ref(&self) -> FontRef {
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }

    fn to_skrifa(&self) -> skrifa::FontRef {
        skrifa::FontRef::new(&self.data).unwrap()
    }

    fn glyph_index(&self, c: char) -> GlyphId {
        self.to_skrifa().charmap().map(c).unwrap().to_u32()
    }

    fn outline(&mut self, size: FontSize, glyph: GlyphId, _subpixel_offset: i32) -> BezPath {
        let font_ref = skrifa::FontRef::from_index(&self.data, 0).unwrap();
        let outlines = font_ref.outline_glyphs();
        let sk_glyph = outlines.get(skrifa::GlyphId::new(glyph as _)).unwrap();
        let hinting = skrifa::outline::HintingInstance::new(
            &outlines,
            Size::new(size as _),
            LocationRef::default(),
            skrifa::outline::HintingMode::Smooth {
                lcd_subpixel: None,
                preserve_linear_metrics: true,
            },
        )
        .unwrap();
        let settings = skrifa::outline::DrawSettings::hinted(&hinting, false);

        let mut pen = BezPen(BezPath::default());
        sk_glyph.draw(settings, &mut pen).unwrap();
        return pen.0;
    }
}
