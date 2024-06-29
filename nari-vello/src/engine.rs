use crate::{
    fxp::fxp6,
    typo::{Font, FontProperties, FontScaled, FontSize, GlyphCache, GlyphId, GlyphKey, TextRun},
};
use parley::{
    fontique::{Attributes, FamilyId, QueryFamily, QueryStatus},
    FontContext, LayoutContext,
};
use skrifa::prelude::*;
use skrifa::MetadataProvider;
use vello::kurbo::{BezPath, Rect};

pub struct Engine {
    font_ctx: FontContext,
    layout_ctx: LayoutContext,
    fonts: Vec<FontData>,
}

impl Engine {
    pub fn new() -> Self {
        let lib = Self {
            font_ctx: FontContext::default(),
            layout_ctx: LayoutContext::default(),
            fonts: Vec::default(),
        };

        lib
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> Font {
        let font_id = self.fonts.len();
        let fonts = self.font_ctx.collection.register_fonts(data);

        self.fonts.push(FontData {
            family_id: fonts[0].0,
        });
        font_id
    }

    pub fn create_font_scaled(
        &mut self,
        font: Font,
        size: FontSize,
        attributes: Attributes,
    ) -> FontScaled {
        let font_data = *self.font(font);

        let mut font_query = self
            .font_ctx
            .collection
            .query(&mut self.font_ctx.source_cache);

        font_query.set_families([QueryFamily::Id(font_data.family_id)]);

        let mut properties = FontProperties {
            ascent: 0.0,
            descent: 0.0,
            height: 0.0,
        };

        font_query.matches_with(|font| {
            let font = skrifa::FontRef::new(font.blob.as_ref()).unwrap();
            let metrics = font.metrics(Size::new(size as _), LocationRef::default());

            properties = FontProperties {
                ascent: fxp6::from_f32(metrics.ascent).f64(),
                descent: fxp6::from_f32(metrics.descent).f64(),
                height: fxp6::from_f32(metrics.ascent - metrics.descent + metrics.leading).f64(),
            };

            QueryStatus::Stop
        });

        FontScaled {
            font,
            size,
            attributes,
            properties,
        }
    }

    fn font(&mut self, id: Font) -> &mut FontData {
        &mut self.fonts[id]
    }

    pub fn build_text_run<S: AsRef<str>>(
        &mut self,
        font: FontScaled,
        text: S,
        glyph_cache: &mut GlyphCache,
    ) -> TextRun {
        use parley::style::StyleProperty;

        let size_px = font.size as f32;
        let font_data = *self.font(font.font);
        let family_name = self
            .font_ctx
            .collection
            .family_name(font_data.family_id)
            .unwrap()
            .to_string();

        let mut builder = self
            .layout_ctx
            .ranged_builder(&mut self.font_ctx, text.as_ref(), 1.0);

        builder.push_default(&StyleProperty::FontStack(parley::style::FontStack::Single(
            parley::style::FontFamily::Named(&family_name), // todo: ugly querying the name here..
        )));
        builder.push_default(&StyleProperty::FontSize(size_px));

        let mut layout = builder.build();
        layout.break_all_lines(None, parley::layout::Alignment::Start);

        let text_run = TextRun { font, layout };

        for line in text_run.layout.lines() {
            for glyph_run in line.glyph_runs() {
                let font_run = glyph_run.run().font();
                let font_ref =
                    skrifa::FontRef::from_index(font_run.data.as_ref(), font_run.index).unwrap();

                for glyph in glyph_run.glyphs() {
                    let glyph_key = GlyphKey { id: glyph.id as _ };

                    glyph_cache
                        .entry((font.size, glyph_key))
                        .or_insert_with(|| {
                            let outlines = font_ref.outline_glyphs();
                            let sk_glyph = outlines.get(skrifa::GlyphId::new(glyph.id)).unwrap();
                            let hinting = skrifa::outline::HintingInstance::new(
                                &outlines,
                                Size::new(size_px),
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
                            pen.0
                        });
                }
            }
        }

        text_run
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FtFontProperties {
    pub ascent: fxp6,
    pub descent: fxp6,
    pub height: fxp6,
}

#[derive(Copy, Clone)]
struct FontData {
    family_id: FamilyId,
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
