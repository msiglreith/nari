use gpu::vk;
use nari_gpu as gpu;
use std::path::Path;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("nari-gpu :: basic")
        .with_inner_size(LogicalSize::new(1440.0f32, 800.0))
        .build(&event_loop)?;

    unsafe {
        let instance = gpu::Instance::new(&window)?;
        let shader_path = Path::new("assets/shaders");
        let mut gpu = gpu::Gpu::new(&instance, shader_path)?;

        let mut size = window.inner_size();
        let mut wsi = gpu::Swapchain::new(
            &instance,
            &gpu,
            size.width,
            size.height,
            vk::PresentModeKHR::IMMEDIATE,
        )?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(_) => {
                        size = window.inner_size();
                        wsi.resize(&gpu, size.width, size.height).unwrap();
                    }
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => (),
                },
                Event::RedrawRequested(_) => {
                    let frame = wsi.acquire().unwrap();
                    let pool = gpu.acquire_pool().unwrap();

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
                            width: size.width,
                            height: size.height,
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
                                color: vk::ClearColorValue { float32: [0.2; 4] },
                            },
                        }],
                    );
                    gpu.cmd_graphics_end(pool);

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
                Event::LoopDestroyed => {
                    wsi.swapchain_fn.destroy_swapchain(wsi.swapchain, None);
                    instance.surface_fn.destroy_surface(instance.surface, None);
                }
                _ => (),
            }
        })
    }
}
