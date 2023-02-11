use nari_gpu as gpu;
use nari_ochre::{Rasterizer as TileRasterizer, Tile, TILE_SIZE};
use std::ops::{Deref, DerefMut, Range};
use std::pin::Pin;
use zeno::PathData;

mod atlas;
mod codicons;
mod engine;
mod fxp;
mod layout;
mod squircle;
pub mod typo;

use self::atlas::{Atlas, AtlasTile};
pub use self::codicons::Codicon;
use self::fxp::fxp6;
pub use self::layout::{Align, Rect};
pub use self::squircle::Squircle;

use self::engine::Engine;

// sRGB (perceptual)
pub type Color = [f32; 4];

fn srgb_to_linear_component(c: f32) -> f32 {
    if c < 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_to_linear(c: Color) -> Color {
    [
        srgb_to_linear_component(c[0]),
        srgb_to_linear_component(c[1]),
        srgb_to_linear_component(c[2]),
        c[3],
    ]
}

fn linear_to_oklab(c: Color) -> Color {
    let l = 0.4122214708 * c[0] + 0.5363325363 * c[1] + 0.0514459929 * c[2];
    let m = 0.2119034982 * c[0] + 0.6806995451 * c[1] + 0.1073969566 * c[2];
    let s = 0.0883024619 * c[0] + 0.2817188376 * c[1] + 0.6299787005 * c[2];

    let l_ = l.powf(1.0 / 3.0);
    let m_ = m.powf(1.0 / 3.0);
    let s_ = s.powf(1.0 / 3.0);

    [
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
        c[3],
    ]
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [i32; 2],
    page: u32,
    tile: u32,
    color: Color,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PipelineMask {
    vertices: gpu::DeviceAddress,
    offset: [i32; 2],
    extent: [i32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PipelineBlit {
    source: gpu::ImageAddress,
}

#[derive(Default)]
struct Scene {
    vertices: Vec<Vertex>,
    atlas_tiles: Vec<AtlasTile>,
    atlas_data: Vec<Tile<u8>>,
}

impl Scene {
    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.hrect(rect, color, color)
    }

    fn path_flip(
        &mut self,
        tiles: &RasterTiles,
        atlas: &Atlas,
        x: i32,
        y: i32,
        color: Color,
        flip_x: bool,
        flip_y: bool,
    ) {
        // allocate tiles in encoder
        for solid_tile in &tiles.solid {
            let dx0 = solid_tile.tx * TILE_SIZE as i32;
            let dx1 = (solid_tile.tx + solid_tile.width) * TILE_SIZE as i32;
            let dy0 = solid_tile.ty * TILE_SIZE as i32;
            let dy1 = (solid_tile.ty + 1) * TILE_SIZE as i32;

            let x0 = if flip_x { x - dx0 } else { x + dx0 };
            let x1 = if flip_x { x - dx1 } else { x + dx1 };
            let y0 = if flip_y { y - dy0 } else { y + dy0 };
            let y1 = if flip_y { y - dy1 } else { y + dy1 };

            let v00 = Vertex {
                pos: [x0, y0],
                page: !0,
                tile: 0,
                color,
            };
            let v01 = Vertex {
                pos: [x0, y1],
                page: !0,
                tile: 0,
                color,
            };
            let v11 = Vertex {
                pos: [x1, y1],
                page: !0,
                tile: 0,
                color,
            };
            let v10 = Vertex {
                pos: [x1, y0],
                page: !0,
                tile: 0,
                color,
            };

            let vertices = [v00, v01, v11, v11, v10, v00];
            self.vertices.extend(vertices);
        }

        for mask_tile in &tiles.mask {
            let dx0 = mask_tile.tx * TILE_SIZE as i32;
            let dx1 = (mask_tile.tx + 1) * TILE_SIZE as i32;
            let dy0 = mask_tile.ty * TILE_SIZE as i32;
            let dy1 = (mask_tile.ty + 1) * TILE_SIZE as i32;

            let x0 = if flip_x { x - dx0 } else { x + dx0 };
            let x1 = if flip_x { x - dx1 } else { x + dx1 };
            let y0 = if flip_y { y - dy0 } else { y + dy0 };
            let y1 = if flip_y { y - dy1 } else { y + dy1 };

            let page_id = mask_tile.atlas_tile.page;
            let page = atlas.pages[page_id as usize].address;

            let tx = mask_tile.atlas_tile.tx as usize;
            let ty = mask_tile.atlas_tile.ty as usize;

            let encode_uv = |u, v| (u as u32) << 16 | v as u32;
            let uv00 = encode_uv(tx * TILE_SIZE, ty * TILE_SIZE);
            let uv01 = encode_uv(tx * TILE_SIZE, (ty + 1) * TILE_SIZE);
            let uv11 = encode_uv((tx + 1) * TILE_SIZE, (ty + 1) * TILE_SIZE);
            let uv10 = encode_uv((tx + 1) * TILE_SIZE, ty * TILE_SIZE);

            let v00 = Vertex {
                pos: [x0, y0],
                page,
                tile: uv00,
                color,
            };
            let v01 = Vertex {
                pos: [x0, y1],
                page,
                tile: uv01,
                color,
            };
            let v11 = Vertex {
                pos: [x1, y1],
                page,
                tile: uv11,
                color,
            };
            let v10 = Vertex {
                pos: [x1, y0],
                page,
                tile: uv10,
                color,
            };

            let vertices = [v00, v01, v11, v11, v10, v00];
            self.vertices.extend(vertices);
        }
    }

    pub fn hrect(&mut self, rect: Rect, c0: Color, c1: Color) {
        let v00 = Vertex {
            pos: [rect.x0, rect.y0],
            page: !0,
            tile: 0,
            color: c0,
        };
        let v01 = Vertex {
            pos: [rect.x0, rect.y1],
            page: !0,
            tile: 0,
            color: c0,
        };
        let v11 = Vertex {
            pos: [rect.x1, rect.y1],
            page: !0,
            tile: 0,
            color: c1,
        };
        let v10 = Vertex {
            pos: [rect.x1, rect.y0],
            page: !0,
            tile: 0,
            color: c1,
        };

        let vertices = [v00, v01, v11, v11, v10, v00];
        self.vertices.extend(vertices);
    }
}

pub struct Canvas {
    gpu: gpu::Gpu,

    engine: Pin<Box<Engine>>,

    canvas: gpu::Image,
    canvas_address: gpu::ImageAddress,
    canvas_view: gpu::ImageView,
    blit_sampler: gpu::Sampler,

    pipeline_mask: gpu::Pipeline<PipelineMask>,
    pipeline_blit: gpu::Pipeline<PipelineBlit>,
    pool_canvas: gpu::Pool,

    rasterizer: TileRasterizer,
    atlas: Atlas,

    glyph_cache: typo::GlyphCache,
    squircle_cache: squircle::SquircleCache,

    scene: Scene,
}

impl Deref for Canvas {
    type Target = gpu::Gpu;
    fn deref(&self) -> &Self::Target {
        &self.gpu
    }
}

impl DerefMut for Canvas {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.gpu
    }
}

impl Canvas {
    pub fn layout_text<S: AsRef<str>>(&mut self, font: typo::FontScaled, text: S) -> typo::TextRun {
        self.engine.layout_text(font, text)
    }

    pub fn build_text_run<S: AsRef<str>>(
        &mut self,
        pool: gpu::Pool,
        font: typo::FontScaled,
        text: S,
    ) -> typo::TextRun {
        let mut rasterizer = Raster {
            gpu: &mut self.gpu,
            atlas: &mut self.atlas,
            scene: &mut self.scene,
            rasterizer: &mut self.rasterizer,
            pool,
        };

        let text_run =
            self.engine
                .build_text_run(font, text, &mut rasterizer, &mut self.glyph_cache);
        rasterizer.upload_atlas();

        text_run
    }

    pub fn build_path(&mut self, pool: gpu::Pool, path: impl PathData) -> RasterTiles {
        let mut rasterizer = Raster {
            gpu: &mut self.gpu,
            atlas: &mut self.atlas,
            scene: &mut self.scene,
            rasterizer: &mut self.rasterizer,
            pool,
        };
        let tiles = rasterizer.render(|raster| {
            path.copy_to(raster);
        });
        rasterizer.upload_atlas();

        tiles
    }

    pub fn squircle(&mut self, squircle: Squircle, color: Color) {
        let radius = squircle
            .radius
            .min(squircle.rect.width() / 2)
            .min(squircle.rect.height() / 2);
        let key = squircle::SquircleKey {
            radius,
            smoothing: fxp6::from_f32(squircle.smoothing),
        };
        let corner = self.squircle_cache.entry(key).or_insert_with(|| {
            let path = squircle::corner(radius as _, key.smoothing.f32());
            let mut rasterizer = Raster {
                gpu: &mut self.gpu,
                atlas: &mut self.atlas,
                scene: &mut self.scene,
                rasterizer: &mut self.rasterizer,
                pool: self.pool_canvas,
            };
            let tiles = rasterizer.render(|raster| {
                (&path).copy_to(raster);
            });
            rasterizer.upload_atlas();
            tiles
        });

        let radius = radius as i32;

        let inner = squircle.rect.margin(layout::Margin {
            left: radius,
            right: radius,
            top: radius,
            bottom: radius,
        });

        // left
        self.scene.rect(
            Rect {
                x0: squircle.rect.x0,
                x1: inner.x0,
                ..inner
            },
            color,
        );
        // right
        self.scene.rect(
            Rect {
                x0: inner.x1,
                x1: squircle.rect.x1,
                ..inner
            },
            color,
        );
        // top
        self.scene.rect(
            Rect {
                y0: squircle.rect.y0,
                y1: squircle.rect.y0 + radius,
                ..inner
            },
            color,
        );
        // bottom
        self.scene.rect(
            Rect {
                y0: squircle.rect.y1 - radius,
                y1: squircle.rect.y1,
                ..inner
            },
            color,
        );
        // center
        self.scene.rect(inner, color);

        // corners
        self.scene
            .path_flip(corner, &self.atlas, inner.x0, inner.y0, color, true, true);
        self.scene
            .path_flip(corner, &self.atlas, inner.x1, inner.y0, color, false, true);
        self.scene
            .path_flip(corner, &self.atlas, inner.x0, inner.y1, color, true, false);
        self.scene
            .path_flip(corner, &self.atlas, inner.x1, inner.y1, color, false, false);
    }

    pub fn char_height(&mut self, font: typo::FontScaled, c: char) -> f32 {
        self.engine.char_height(font, c)
    }

    pub fn text<S: AsRef<str>>(&mut self, font: typo::FontScaled, pen: typo::Pen, text: S) {
        let run = self.build_text_run(self.pool_canvas, font, text);
        self.text_run(pen, &run);
    }

    pub fn text_run(&mut self, pen: typo::Pen, text_run: &typo::TextRun) {
        let px = pen.x + text_run.offset_x(pen.align_x);

        for cluster in &text_run.clusters {
            for glyph in &cluster.glyphs {
                let key = typo::GlyphKey {
                    id: glyph.id,
                    offset: glyph.offset.fract(),
                };
                let tiles = self
                    .glyph_cache
                    .get(&(text_run.font.size, key))
                    .expect(&format!("missing {:?}", (text_run.font.size, key)));
                let advance = px + glyph.offset.trunc().i32();

                for solid_tile in &tiles.solid {
                    let x0 = solid_tile.tx * TILE_SIZE as i32 + advance;
                    let x1 = (solid_tile.tx + solid_tile.width) * TILE_SIZE as i32 + pen.y;
                    let y0 = solid_tile.ty * TILE_SIZE as i32 + advance;
                    let y1 = (solid_tile.ty - 1) * TILE_SIZE as i32 + pen.y;

                    let v00 = Vertex {
                        pos: [x0, y0],
                        page: !0,
                        tile: 0,
                        color: pen.color,
                    };
                    let v01 = Vertex {
                        pos: [x0, y1],
                        page: !0,
                        tile: 0,
                        color: pen.color,
                    };
                    let v11 = Vertex {
                        pos: [x1, y1],
                        page: !0,
                        tile: 0,
                        color: pen.color,
                    };
                    let v10 = Vertex {
                        pos: [x1, y0],
                        page: !0,
                        tile: 0,
                        color: pen.color,
                    };

                    let vertices = [v00, v01, v11, v11, v10, v00];
                    self.scene.vertices.extend(vertices);
                }

                for mask_tile in &tiles.mask {
                    let x0 = mask_tile.tx * TILE_SIZE as i32 + advance;
                    let x1 = (mask_tile.tx + 1) * TILE_SIZE as i32 + advance;
                    let y0 = -mask_tile.ty * TILE_SIZE as i32 + pen.y;
                    let y1 = -(mask_tile.ty + 1) * TILE_SIZE as i32 + pen.y;

                    let page_id = mask_tile.atlas_tile.page;
                    let page = self.atlas.pages[page_id as usize].address;

                    let tx = mask_tile.atlas_tile.tx as usize;
                    let ty = mask_tile.atlas_tile.ty as usize;

                    let encode_uv = |u, v| (u as u32) << 16 | v as u32;
                    let uv00 = encode_uv(tx * TILE_SIZE, ty * TILE_SIZE);
                    let uv01 = encode_uv(tx * TILE_SIZE, (ty + 1) * TILE_SIZE);
                    let uv11 = encode_uv((tx + 1) * TILE_SIZE, (ty + 1) * TILE_SIZE);
                    let uv10 = encode_uv((tx + 1) * TILE_SIZE, ty * TILE_SIZE);

                    let v00 = Vertex {
                        pos: [x0, y0],
                        page,
                        tile: uv00,
                        color: pen.color,
                    };
                    let v01 = Vertex {
                        pos: [x0, y1],
                        page,
                        tile: uv01,
                        color: pen.color,
                    };
                    let v11 = Vertex {
                        pos: [x1, y1],
                        page,
                        tile: uv11,
                        color: pen.color,
                    };
                    let v10 = Vertex {
                        pos: [x1, y0],
                        page,
                        tile: uv10,
                        color: pen.color,
                    };

                    let vertices = [v00, v01, v11, v11, v10, v00];
                    self.scene.vertices.extend(vertices);
                }
            }
        }
    }

    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.scene.rect(rect, color)
    }

    pub fn hrect(&mut self, rect: Rect, c0: Color, c1: Color) {
        self.scene.hrect(rect, c0, c1)
    }

    pub fn path(&mut self, tiles: &RasterTiles, x: i32, y: i32, color: Color) {
        self.scene
            .path_flip(tiles, &self.atlas, x, y, color, false, false);
    }

    pub unsafe fn composition_begin(&mut self, pool: gpu::Pool) {
        self.pool_canvas = pool;
    }

    pub unsafe fn composition_end(
        &mut self,
        area: gpu::Rect2D,
        attachment: gpu::ImageView,
        load: gpu::vk::AttachmentLoadOp,
        clear_srgb: Color,
    ) {
        Raster {
            gpu: &mut self.gpu,
            atlas: &mut self.atlas,
            scene: &mut self.scene,
            rasterizer: &mut self.rasterizer,
            pool: self.pool_canvas,
        }
        .upload_atlas();

        let clear = gpu::vk::ClearValue {
            color: gpu::vk::ClearColorValue {
                float32: linear_to_oklab(srgb_to_linear(clear_srgb)),
            },
        };

        let vertices = (!self.scene.vertices.is_empty()).then(|| {
            self.gpu
                .create_buffer_gpu(
                    "canvas::vertices",
                    std::mem::size_of::<Vertex>() * self.scene.vertices.len(),
                    gpu::BufferUsageFlags::STORAGE_BUFFER,
                    gpu::BufferInit::Host {
                        pool: self.pool_canvas,
                        data: gpu::as_u8_slice(&self.scene.vertices),
                    },
                )
                .unwrap()
        });

        self.gpu.cmd_set_viewport(
            self.pool_canvas.cmd_buffer,
            0,
            &[gpu::vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: area.extent.width as _,
                height: area.extent.height as _,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        self.gpu.cmd_set_scissor(
            self.pool_canvas.cmd_buffer,
            0,
            &[gpu::Rect2D {
                offset: gpu::Offset2D { x: 0, y: 0 },
                extent: area.extent,
            }],
        );

        // Canvas target: blit input -> color output
        self.gpu.cmd_barriers(
            self.pool_canvas,
            &[gpu::MemoryBarrier::full()],
            &[gpu::ImageBarrier {
                image: self.canvas.aspect(gpu::vk::ImageAspectFlags::COLOR),
                src: gpu::ImageAccess {
                    access: gpu::Access::SHADER_READ,
                    stage: gpu::Stage::FRAGMENT_SHADER,
                    layout: gpu::ImageLayout::READ_ONLY_OPTIMAL,
                },
                dst: gpu::ImageAccess {
                    access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                    stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                },
            }],
        );

        self.gpu.cmd_graphics_begin(
            self.pool_canvas,
            area,
            &[gpu::GraphicsAttachment {
                image_view: self.canvas_view,
                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                load: gpu::vk::AttachmentLoadOp::CLEAR,
                store: gpu::vk::AttachmentStoreOp::STORE,
                clear,
            }],
        );

        if let Some(vertices) = vertices {
            self.gpu.cmd_graphics_draw(
                self.pool_canvas,
                self.pipeline_mask,
                PipelineMask {
                    vertices: self.gpu.buffer_address(&vertices),
                    offset: [area.offset.x, area.offset.y],
                    extent: [area.extent.width as _, area.extent.height as _],
                },
                &[gpu::GraphicsDraw {
                    vertex_count: self.scene.vertices.len() as _,
                    instance_count: 1,
                    first_vertex: 0,
                    first_instance: 0,
                }],
            );

            self.gpu.cmd_retire_buffer(self.pool_canvas, vertices);
        }

        self.gpu.cmd_graphics_end(self.pool_canvas);

        // Canvas target: color output -> blit input
        self.gpu.cmd_barriers(
            self.pool_canvas,
            &[gpu::MemoryBarrier::full()],
            &[gpu::ImageBarrier {
                image: self.canvas.aspect(gpu::vk::ImageAspectFlags::COLOR),
                src: gpu::ImageAccess {
                    access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                    stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                },
                dst: gpu::ImageAccess {
                    access: gpu::Access::SHADER_READ,
                    stage: gpu::Stage::FRAGMENT_SHADER,
                    layout: gpu::ImageLayout::READ_ONLY_OPTIMAL,
                },
            }],
        );

        // blit internal to output buffer
        let clear = gpu::vk::ClearValue {
            color: gpu::vk::ClearColorValue {
                float32: clear_srgb,
            },
        };
        self.gpu.cmd_graphics_begin(
            self.pool_canvas,
            area,
            &[gpu::GraphicsAttachment {
                image_view: attachment,
                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                load,
                store: gpu::vk::AttachmentStoreOp::STORE,
                clear,
            }],
        );
        self.gpu.cmd_graphics_draw(
            self.pool_canvas,
            self.pipeline_blit,
            PipelineBlit {
                source: self.canvas_address,
            },
            &[gpu::GraphicsDraw {
                vertex_count: 3,
                instance_count: 1,
                first_vertex: 0,
                first_instance: 0,
            }],
        );
        self.gpu.cmd_graphics_end(self.pool_canvas);

        self.scene.vertices.clear();
    }
}

const CANVAS_FORMAT: gpu::vk::Format = gpu::vk::Format::R32G32B32A32_SFLOAT;

impl Canvas {
    pub unsafe fn new(
        mut gpu: gpu::Gpu,
        width: u32,
        height: u32,
        output_format: gpu::Format,
    ) -> Self {
        let blit_sampler = gpu
            .create_sampler(
                &gpu::vk::SamplerCreateInfo::default()
                    .mag_filter(gpu::vk::Filter::LINEAR)
                    .min_filter(gpu::vk::Filter::LINEAR)
                    .address_mode_u(gpu::vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(gpu::vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
            .unwrap();

        let canvas = gpu
            .create_image_gpu(
                "canvas::target",
                &gpu::ImageDesc {
                    ty: gpu::ImageType::TYPE_2D,
                    format: CANVAS_FORMAT,
                    extent: gpu::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                    usage: gpu::ImageUsageFlags::COLOR_ATTACHMENT | gpu::ImageUsageFlags::SAMPLED,
                    mip_levels: 1,
                    array_layers: 1,
                    samples: 1,
                },
                gpu::ImageInit::None,
            )
            .unwrap();

        let canvas_aspect = canvas.aspect(gpu::vk::ImageAspectFlags::COLOR);
        let view_desc = gpu::vk::ImageViewCreateInfo::default()
            .image(canvas_aspect.image)
            .view_type(gpu::vk::ImageViewType::TYPE_2D)
            .format(CANVAS_FORMAT)
            .subresource_range(canvas_aspect.range);
        let canvas_view = gpu.create_image_view(&view_desc, None).unwrap();

        let canvas_address = gpu.sampled_image_address(canvas_view, blit_sampler);

        let target = gpu::GraphicsOutputColor {
            format: CANVAS_FORMAT,
            blend: gpu::GraphicsOutputBlend::Enable {
                color: gpu::GraphicsBlendEq::OVER,
                alpha: gpu::GraphicsBlendEq::OVER,
            },
        };

        let mask_vert = gpu.create_shader("canvas_mask.vert.spv").unwrap();
        let mask_frag = gpu.create_shader("canvas_mask.frag.spv").unwrap();
        let pipeline_mask = gpu
            .create_graphics_pipeline::<PipelineMask>(
                "canvas::mask",
                gpu::GraphicsPrimitives {
                    shader: gpu::GraphicsPrimitivesShader::Vertex {
                        shader: gpu::ShaderEntry {
                            module: mask_vert,
                            entry: "main",
                        },
                    },
                    topology: gpu::PrimitiveTopology::TRIANGLE_LIST,
                    restart: false,
                },
                gpu::vk::PipelineRasterizationStateCreateInfo::default().line_width(1.0),
                gpu::ShaderEntry {
                    module: mask_frag,
                    entry: "main",
                },
                &[target],
            )
            .unwrap();

        let target = gpu::GraphicsOutputColor {
            format: output_format,
            blend: gpu::GraphicsOutputBlend::Disable,
        };

        let blit_vert = gpu.create_shader("canvas_blit.vert.spv").unwrap();
        let blit_frag = gpu.create_shader("canvas_blit.frag.spv").unwrap();
        let pipeline_blit = gpu
            .create_graphics_pipeline::<PipelineBlit>(
                "canvas::blit",
                gpu::GraphicsPrimitives {
                    shader: gpu::GraphicsPrimitivesShader::Vertex {
                        shader: gpu::ShaderEntry {
                            module: blit_vert,
                            entry: "main",
                        },
                    },
                    topology: gpu::PrimitiveTopology::TRIANGLE_LIST,
                    restart: false,
                },
                gpu::vk::PipelineRasterizationStateCreateInfo::default().line_width(1.0),
                gpu::ShaderEntry {
                    module: blit_frag,
                    entry: "main",
                },
                &[target],
            )
            .unwrap();

        let atlas_sampler = gpu
            .create_sampler(
                &gpu::vk::SamplerCreateInfo::default()
                    .mag_filter(gpu::vk::Filter::LINEAR)
                    .min_filter(gpu::vk::Filter::LINEAR)
                    .address_mode_u(gpu::vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(gpu::vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .unnormalized_coordinates(true),
                None,
            )
            .unwrap();

        let atlas = Atlas {
            pages: Vec::default(),
            sampler: atlas_sampler,
        };

        Self {
            engine: Engine::new(),
            rasterizer: TileRasterizer::default(),
            atlas,
            gpu,
            canvas,
            canvas_view,
            canvas_address,
            blit_sampler,
            pool_canvas: gpu::Pool::null(),
            pipeline_mask,
            pipeline_blit,
            scene: Scene::default(),
            glyph_cache: Default::default(),
            squircle_cache: Default::default(),
        }
    }

    pub unsafe fn resize(&mut self, width: u32, height: u32) {
        let mut canvas = self
            .gpu
            .create_image_gpu(
                "canvas::target",
                &gpu::ImageDesc {
                    ty: gpu::ImageType::TYPE_2D,
                    format: CANVAS_FORMAT,
                    extent: gpu::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                    usage: gpu::ImageUsageFlags::COLOR_ATTACHMENT | gpu::ImageUsageFlags::SAMPLED,
                    mip_levels: 1,
                    array_layers: 1,
                    samples: 1,
                },
                gpu::ImageInit::None,
            )
            .unwrap();

        let canvas_aspect = canvas.aspect(gpu::vk::ImageAspectFlags::COLOR);
        let view_desc = gpu::vk::ImageViewCreateInfo::default()
            .image(canvas_aspect.image)
            .view_type(gpu::vk::ImageViewType::TYPE_2D)
            .format(CANVAS_FORMAT)
            .subresource_range(canvas_aspect.range);
        let mut canvas_view = self.gpu.create_image_view(&view_desc, None).unwrap();
        let mut canvas_address = self
            .gpu
            .sampled_image_address(canvas_view, self.blit_sampler);

        std::mem::swap(&mut self.canvas, &mut canvas);
        std::mem::swap(&mut self.canvas_view, &mut canvas_view);
        std::mem::swap(&mut self.canvas_address, &mut canvas_address);

        // destroy old one
        self.gpu.retire_image_view(canvas_view).unwrap();
        self.gpu.retire_image(canvas).unwrap();
        // todo: destroy canvas address and image view
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> typo::Font {
        self.engine.create_font(data)
    }

    pub fn create_font_scaled(
        &mut self,
        font: typo::Font,
        size: typo::FontSize,
    ) -> typo::FontScaled {
        self.engine.create_font_scaled(font, size)
    }
}

struct Raster<'a> {
    gpu: &'a mut gpu::Gpu,
    atlas: &'a mut Atlas,
    scene: &'a mut Scene,
    rasterizer: &'a mut TileRasterizer,

    pool: gpu::Pool,
}

impl Raster<'_> {
    fn render<F: FnMut(&mut TileRasterizer)>(&mut self, mut raster: F) -> RasterTiles {
        let mut path_encoder = PathEncoder {
            tiles: RasterTiles::default(),
            gpu: &mut self.gpu,
            atlas: &mut self.atlas,
            scene: &mut self.scene,
            pool: self.pool,
        };
        self.rasterizer.begin();
        (raster)(&mut self.rasterizer);
        self.rasterizer.end(&mut path_encoder);
        path_encoder.tiles
    }

    fn upload_atlas(&mut self) {
        let num_tiles = self.scene.atlas_tiles.len();
        if num_tiles == 0 {
            return;
        }

        let mut atlas_buffer = unsafe {
            self.gpu
                .create_buffer_upload(
                    "canvas::atlas-cpu",
                    std::mem::size_of::<Tile<u8>>() * num_tiles,
                    gpu::BufferUsageFlags::TRANSFER_SRC,
                )
                .unwrap()
        };

        {
            let dst: &mut [Tile<u8>] =
                bytemuck::cast_slice_mut(atlas_buffer.allocation.mapped_slice_mut().unwrap());
            dst[..num_tiles].clone_from_slice(&self.scene.atlas_data);
        }

        for (i, tile) in self.scene.atlas_tiles.iter().enumerate() {
            let region = gpu::BufferImageCopy {
                buffer_offset: (i * std::mem::size_of::<Tile<u8>>()) as _,
                buffer_row_length: 0,
                buffer_image_height: 0,
                image_subresource: gpu::vk::ImageSubresourceLayers {
                    aspect_mask: gpu::vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_offset: gpu::Offset3D {
                    x: tile.tx as i32 * TILE_SIZE as i32,
                    y: tile.ty as i32 * TILE_SIZE as i32,
                    z: 0,
                },
                image_extent: gpu::Extent3D {
                    width: TILE_SIZE as u32,
                    height: TILE_SIZE as u32,
                    depth: 1,
                },
            };
            unsafe {
                self.gpu.cmd_copy_buffer_to_image(
                    self.pool.cmd_buffer,
                    atlas_buffer.buffer,
                    self.atlas.pages[tile.page as usize].image.image,
                    gpu::ImageLayout::GENERAL, // todo: transfer_dst_optimal?
                    &[region],
                );
            }
        }

        self.gpu.cmd_retire_buffer(self.pool, atlas_buffer);

        self.scene.atlas_data.clear();
        self.scene.atlas_tiles.clear();
    }
}

#[derive(Debug)]
pub struct RasterTileSolid {
    pub tx: i32,
    pub ty: i32,
    pub width: i32,
}

#[derive(Debug)]
pub struct RasterTileMask {
    pub tx: i32,
    pub ty: i32,
    pub atlas_tile: AtlasTile,
}

pub struct PathEncoder<'a> {
    tiles: RasterTiles,

    gpu: &'a mut gpu::Gpu,
    atlas: &'a mut Atlas,
    pool: gpu::Pool,

    scene: &'a mut Scene,
}

#[derive(Debug, Default)]
pub struct RasterTiles {
    pub solid: Vec<RasterTileSolid>,
    pub mask: Vec<RasterTileMask>,
}

impl nari_ochre::Encoder for PathEncoder<'_> {
    fn solid(&mut self, y: i16, x: Range<i16>) {
        self.tiles.solid.push(RasterTileSolid {
            tx: x.start as _,
            ty: y as _,
            width: (x.end - x.start) as _,
        });
    }

    fn mask(&mut self, y: i16, x: i16, mask: &Tile<u8>) {
        let atlas_tile = self.atlas.allocate(self.gpu, self.pool);
        self.tiles.mask.push(RasterTileMask {
            tx: x as _,
            ty: y as _,
            atlas_tile,
        });
        self.scene.atlas_tiles.push(atlas_tile);
        self.scene.atlas_data.push(mask.clone());
    }
}
