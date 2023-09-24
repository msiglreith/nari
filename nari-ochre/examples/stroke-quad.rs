use gpu::vk;
use nari_gpu as gpu;
use nari_ochre::euler::*;
use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use std::path::Path;
use zeno::Point;

#[repr(C)]
#[derive(Copy, Clone)]
struct StrokeQuadVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct StrokeQuadParams {
    vertices: gpu::DeviceAddress,
    offset: [f32; 2],
    extent: [f32; 2],
}

struct Border;
impl Border {
    const MARGIN: i32 = 5;
    fn hittest(surface: nari_platform::Surface, x: i32, y: i32) -> Option<SurfaceArea> {
        if surface.is_maximized() {
            return None;
        }

        let Extent { width, height } = surface.extent();
        let width = width as i32;
        let height = height as i32;

        if x <= Self::MARGIN {
            return if y <= Self::MARGIN {
                Some(SurfaceArea::TopLeft)
            } else if y >= height - Self::MARGIN {
                Some(SurfaceArea::BottomLeft)
            } else {
                Some(SurfaceArea::Left)
            };
        }

        if x >= width - Self::MARGIN {
            return if y <= Self::MARGIN {
                Some(SurfaceArea::TopRight)
            } else if y >= height - Self::MARGIN {
                Some(SurfaceArea::BottomRight)
            } else {
                Some(SurfaceArea::Right)
            };
        }

        if y <= Self::MARGIN {
            return Some(SurfaceArea::Top);
        }
        if y >= height - Self::MARGIN {
            return Some(SurfaceArea::Bottom);
        }

        None
    }
}

fn main() -> anyhow::Result<()> {
    let platform = Platform::new();

    unsafe {
        let instance = gpu::Instance::with_surface(&platform.surface)?;
        let shader_path = Path::new("assets/shaders");
        let mut gpu = gpu::Gpu::new(&instance, shader_path)?;

        let size = platform.surface.extent();
        let mut wsi = gpu::Swapchain::new(
            &instance,
            &gpu,
            size.width as _,
            size.height as _,
            vk::PresentModeKHR::IMMEDIATE,
        )?;

        let stroke_quad_vs = gpu.create_shader("stroke_quad.vert.spv")?;
        let stroke_quad_fs = gpu.create_shader("stroke_quad.frag.spv")?;
        let stroke_quad: gpu::Pipeline<StrokeQuadParams> = gpu.create_graphics_pipeline(
            "stroke-quad",
            gpu::GraphicsPrimitives {
                shader: gpu::GraphicsPrimitivesShader::Vertex {
                    shader: gpu::ShaderEntry {
                        module: stroke_quad_vs,
                        entry: "main",
                    },
                },
                topology: gpu::PrimitiveTopology::TRIANGLE_LIST,
                restart: false,
            },
            gpu::vk::PipelineRasterizationStateCreateInfo::default(),
            gpu::ShaderEntry {
                module: stroke_quad_fs,
                entry: "main",
            },
            &[gpu::GraphicsOutputColor {
                format: gpu::vk::Format::R8G8B8A8_SRGB,
                blend: gpu::GraphicsOutputBlend::Disable,
            }],
        )?;

        platform.run(move |event_loop, event| {
            match event {
                Event::Resize(extent) => {
                    wsi.resize(&gpu, extent.width as _, extent.height as _)
                        .unwrap();
                    event_loop.surface.redraw();
                }
                Event::Hittest { x, y, area } => {
                    if let Some(hit_area) = Border::hittest(event_loop.surface, x, y) {
                        *area = hit_area;
                    }
                }

                Event::Paint => {
                    let size = event_loop.surface.extent();
                    let frame = wsi.acquire().unwrap();
                    let pool = gpu.acquire_pool().unwrap();

                    let euler = Euler {
                        p: Point::new(0.0, 0.0),
                        scale: 300.0,
                        k: [0.0, 1.0, -6.0],
                    };
                    let width = 200.0;
                    let offset = width;

                    let mut vertices: Vec<StrokeQuadVertex> = Vec::new();

                    let mut prev_p = euler.p + euler_normal(euler, 0.0, offset);
                    let mut prev_n = euler.p + euler_normal(euler, 0.0, -offset);

                    let dt = 1.0 / euler.scale;
                    let mut t = 0.0;
                    while t < 1.0 {
                        let prev_t = t as f32;
                        t = (t + dt).min(1.0);
                        let p = euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset);
                        let n = euler_eval(euler, 0.0, t) + euler_normal(euler, t, -offset);

                        vertices.extend(&[
                            StrokeQuadVertex {
                                pos: [prev_p.x, prev_p.y],
                                uv: [prev_t, 0.0],
                            },
                            StrokeQuadVertex {
                                pos: [prev_n.x, prev_n.y],
                                uv: [prev_t, 0.0],
                            },
                            StrokeQuadVertex {
                                pos: [p.x, p.y],
                                uv: [t as f32, 0.0],
                            },
                            StrokeQuadVertex {
                                pos: [n.x, n.y],
                                uv: [t as f32, 0.0],
                            },
                            StrokeQuadVertex {
                                pos: [p.x, p.y],
                                uv: [t as f32, 0.0],
                            },
                            StrokeQuadVertex {
                                pos: [prev_n.x, prev_n.y],
                                uv: [prev_t, 0.0],
                            },
                        ]);

                        prev_p = p;
                        prev_n = n;
                    }

                    let quads = gpu
                        .create_buffer_gpu(
                            "quads",
                            std::mem::size_of::<StrokeQuadVertex>() * vertices.len(),
                            gpu::BufferUsageFlags::VERTEX_BUFFER,
                            gpu::BufferInit::Host {
                                pool: pool,
                                data: gpu::as_u8_slice(&vertices),
                            },
                        )
                        .unwrap();

                    gpu.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess {
                                access: gpu::Access::NONE,
                                stage: gpu::Stage::NONE,
                                layout: gpu::ImageLayout::UNDEFINED,
                            },
                            dst: gpu::ImageAccess {
                                access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            },
                        }],
                    );

                    let area = vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: vk::Extent2D {
                            width: size.width as _,
                            height: size.height as _,
                        },
                    };
                    gpu.cmd_set_viewport(
                        pool.cmd_buffer,
                        0,
                        &[vk::Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: size.width as _,
                            height: size.height as _,
                            min_depth: 0.0,
                            max_depth: 1.0,
                        }],
                    );
                    gpu.cmd_set_scissor(pool.cmd_buffer, 0, &[area]);
                    gpu.cmd_graphics_begin(
                        pool,
                        area,
                        &[gpu::GraphicsAttachment {
                            image_view: wsi.frame_rtvs[frame.id],
                            layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            load: vk::AttachmentLoadOp::CLEAR,
                            store: vk::AttachmentStoreOp::STORE,
                            clear: vk::ClearValue {
                                color: vk::ClearColorValue { float32: [0.0; 4] },
                            },
                        }],
                    );
                    gpu.cmd_graphics_draw(
                        pool,
                        stroke_quad,
                        StrokeQuadParams {
                            vertices: gpu.buffer_address(&quads),
                            offset: [-size.width as f32 / 2.0, -size.height as f32 / 2.0],
                            extent: [size.width as f32, size.height as f32],
                        },
                        &[gpu::GraphicsDraw {
                            vertex_count: vertices.len() as _,
                            first_vertex: 0,
                            instance_count: 1,
                            first_instance: 0,
                        }],
                    );

                    gpu.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess {
                                access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            },
                            dst: gpu::ImageAccess {
                                access: gpu::Access::NONE,
                                stage: gpu::Stage::NONE,
                                layout: gpu::ImageLayout::PRESENT_SRC_KHR,
                            },
                        }],
                    );

                    gpu.submit_pool(
                        pool,
                        gpu::Submit {
                            waits: &[gpu::SemaphoreSubmit {
                                semaphore: frame.acquire,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                            }],
                            signals: &[gpu::SemaphoreSubmit {
                                semaphore: frame.present,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                            }],
                        },
                    )
                    .unwrap();

                    wsi.present(&gpu, frame).unwrap();
                }
                _ => (),
            }

            ControlFlow::Continue
        });
    }

    Ok(())
}
