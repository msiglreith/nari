use crate::{
    fxp::fxp6,
    layout::Rect,
    typo::{
        Font, FontProperties, FontScaled, FontSize, GlyphCache, GlyphId, GlyphKey, TextRun,
        TextRunGlyph, TextRunGraphemeCluster,
    },
    Raster,
};
use libc::{self, c_long, c_void, size_t};
use nari_freetype as ft_sys;
use nari_ochre::Rasterizer;
use std::{pin::Pin, ptr};
use swash::{shape::ShapeContext, CacheKey, FontRef};
use zeno::{PathBuilder, Point};

extern "C" fn alloc_library(_memory: ft_sys::FT_Memory, size: c_long) -> *mut c_void {
    unsafe { libc::malloc(size as size_t) }
}

extern "C" fn free_library(_memory: ft_sys::FT_Memory, block: *mut c_void) {
    unsafe { libc::free(block) }
}

extern "C" fn realloc_library(
    _memory: ft_sys::FT_Memory,
    _cur_size: c_long,
    new_size: c_long,
    block: *mut c_void,
) -> *mut c_void {
    unsafe { libc::realloc(block, new_size as size_t) }
}

pub(crate) struct Engine {
    library: ft_sys::FT_Library,
    memory: ft_sys::FT_MemoryRec,
    shaper: ShapeContext,
    fonts: Vec<FontData>,
}

impl Engine {
    pub fn new() -> Pin<Box<Self>> {
        let lib = Self {
            library: ptr::null_mut(),
            memory: ft_sys::FT_MemoryRec {
                user: ptr::null_mut() as *mut c_void,
                alloc: alloc_library,
                free: free_library,
                realloc: realloc_library,
            },
            shaper: ShapeContext::new(),
            fonts: Vec::default(),
        };

        let mut lib = Box::pin(lib);
        unsafe {
            let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut lib);
            let mut_ref = Pin::get_unchecked_mut(mut_ref);
            assert_eq!(
                ft_sys::FT_New_Library(&mut mut_ref.memory, &mut mut_ref.library),
                ft_sys::FT_Err_Ok
            );
            ft_sys::FT_Add_Default_Modules(lib.library);
        }

        lib
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> Font {
        let font_id = self.fonts.len();

        // freetype
        let mut face = ptr::null_mut();
        unsafe {
            assert_eq!(
                ft_sys::FT_New_Memory_Face(
                    self.library,
                    data.as_ptr(),
                    data.len() as _,
                    0,
                    &mut face
                ),
                ft_sys::FT_Err_Ok
            );
        }

        // swash
        let font_ref = FontRef::from_index(&data, 0).unwrap();

        self.fonts.push(FontData {
            key: font_ref.key,
            offset: font_ref.offset,
            face,
            data,
        });
        font_id
    }

    pub fn create_font_scaled(&mut self, font: Font, size: FontSize) -> FontScaled {
        let font_ref = self.font(font);
        font_ref.scale(size);
        let metrics = font_ref.properties();

        let properties = FontProperties {
            ascent: metrics.ascent.i32(),
            descent: metrics.descent.i32(),
            height: metrics.height.i32(),
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

    pub fn char_extent(&mut self, font: FontScaled, c: char) -> Rect {
        let font_ref = self.font(font.font);
        font_ref.scale(font.size);

        unsafe {
            assert_eq!(
                ft_sys::FT_Load_Char(
                    font_ref.face,
                    c as _,
                    ft_sys::FT_LOAD_NO_BITMAP | ft_sys::FT_LOAD_TARGET_NORMAL
                ),
                ft_sys::FT_Err_Ok
            );
        }

        let x = unsafe { (*(*font_ref.face).glyph).metrics.horiBearingX };
        let y = unsafe { (*(*font_ref.face).glyph).metrics.horiBearingY };
        let width = unsafe { (*(*font_ref.face).glyph).metrics.width };
        let height = unsafe { (*(*font_ref.face).glyph).metrics.height };

        Rect {
            x0: fxp6::new(x).f32().round() as i32,
            y0: fxp6::new(y - height).f32().round() as i32,
            x1: fxp6::new(x + width).f32().round() as i32,
            y1: fxp6::new(y).f32().round() as i32,
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
                advances: advance.i32()..0,
            };

            for glyph in cluster.glyphs {
                cls.glyphs.push(TextRunGlyph {
                    id: glyph.id as _,
                    offset: advance,
                });
                advance.0 += fxp6::from_f32(glyph.advance).0;
            }
            cls.advances.end = advance.i32();

            text_run.clusters.push(cls);
        });
        text_run.width = advance.f32(); // todo: includes bearing, not tight /:
        text_run
    }

    pub(crate) fn build_text_run<S: AsRef<str>>(
        &mut self,
        font: FontScaled,
        text: S,
        rasterizer: &mut Raster,
        glyph_cache: &mut GlyphCache,
    ) -> TextRun {
        let text_run = self.layout_text(font, text);

        let font_ref = self.font(font.font);
        font_ref.scale(font.size);

        for cluster in &text_run.clusters {
            for glyph in &cluster.glyphs {
                let subpixel_offset = glyph.offset.fract();
                let glyph_key = GlyphKey {
                    id: glyph.id,
                    offset: subpixel_offset,
                };

                glyph_cache
                    .entry((font.size, glyph_key))
                    .or_insert_with(|| {
                        rasterizer.render(|raster| {
                            font_ref.outline(glyph.id, subpixel_offset.0, raster);
                        })
                    });
            }
        }

        text_run
    }

    pub(crate) fn build_glyph(
        &mut self,
        font: FontScaled,
        c: char,
        rasterizer: &mut Raster,
        glyph_cache: &mut GlyphCache,
    ) -> TextRunGlyph {
        let font_ref = self.font(font.font);
        font_ref.scale(font.size);

        let glyph_key = GlyphKey {
            id: font_ref.glyph_index(c),
            offset: fxp6::new(0),
        };

        glyph_cache
            .entry((font.size, glyph_key))
            .or_insert_with(|| {
                rasterizer.render(|raster| {
                    font_ref.outline(glyph_key.id, glyph_key.offset.0, raster);
                })
            });

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

    // freetype
    face: ft_sys::FT_Face,

    // swash
    offset: u32,
    key: CacheKey,
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

    // freetype
    fn scale(&self, size_px: u32) {
        unsafe {
            assert_eq!(
                ft_sys::FT_Set_Pixel_Sizes(self.face, 0, size_px),
                ft_sys::FT_Err_Ok
            );
        }
    }

    fn properties(&self) -> FtFontProperties {
        let face = unsafe { &*self.face };
        let size = unsafe { &*face.size }; // todo: requires earlier call of `scale`

        FtFontProperties {
            ascent: fxp6::new(size.metrics.ascender),
            descent: fxp6::new(size.metrics.descender),
            height: fxp6::new(size.metrics.height),
        }
    }

    fn glyph_index(&self, c: char) -> GlyphId {
        unsafe { ft_sys::FT_Get_Char_Index(self.face, c as _) }
    }

    fn outline(&mut self, glyph: GlyphId, subpixel_offset: i32, rasterizer: &mut Rasterizer) {
        let glyph = self.load_glyph(glyph);

        let outline_fn = ft_sys::FT_Outline_Funcs {
            move_to: path_move_to,
            conic_to: path_conic_to,
            cubic_to: path_cubic_to,
            line_to: path_line_to,
            shift: 0,
            delta: 0,
        };
        unsafe {
            ft_sys::FT_Outline_Translate(&mut glyph.outline as *mut _, subpixel_offset, 0);
            assert_eq!(
                ft_sys::FT_Outline_Decompose(
                    &mut glyph.outline as *mut _,
                    &outline_fn,
                    rasterizer as *mut _ as *mut _
                ),
                ft_sys::FT_Err_Ok
            );
        }
    }

    fn load_glyph(&mut self, glyph: GlyphId) -> &mut ft_sys::FT_GlyphSlotRec {
        unsafe {
            assert_eq!(
                ft_sys::FT_Load_Glyph(
                    self.face,
                    glyph,
                    ft_sys::FT_LOAD_NO_BITMAP | ft_sys::FT_LOAD_TARGET_NORMAL
                ),
                ft_sys::FT_Err_Ok
            );
            &mut *(*self.face).glyph
        }
    }
}

fn ft_vector_as_point(v: *const ft_sys::FT_Vector) -> Point {
    unsafe {
        Point {
            x: fxp6::new((*v).x).f32(),
            y: fxp6::new((*v).y).f32(),
        }
    }
}

extern "C" fn path_line_to(to: *const ft_sys::FT_Vector, user: *mut c_void) -> i32 {
    let path: &mut Rasterizer = unsafe { &mut *(user as *mut _) };
    path.line_to(ft_vector_as_point(to));
    0
}

extern "C" fn path_conic_to(
    c: *const ft_sys::FT_Vector,
    to: *const ft_sys::FT_Vector,
    user: *mut c_void,
) -> i32 {
    let path: &mut Rasterizer = unsafe { &mut *(user as *mut _) };
    path.quad_to(ft_vector_as_point(c), ft_vector_as_point(to));
    0
}

extern "C" fn path_cubic_to(
    c0: *const ft_sys::FT_Vector,
    c1: *const ft_sys::FT_Vector,
    to: *const ft_sys::FT_Vector,
    user: *mut c_void,
) -> i32 {
    let path: &mut Rasterizer = unsafe { &mut *(user as *mut _) };
    path.curve_to(
        ft_vector_as_point(c0),
        ft_vector_as_point(c1),
        ft_vector_as_point(to),
    );
    0
}

extern "C" fn path_move_to(to: *const ft_sys::FT_Vector, user: *mut c_void) -> i32 {
    let path: &mut Rasterizer = unsafe { &mut *(user as *mut _) };
    path.move_to(ft_vector_as_point(to));
    0
}
