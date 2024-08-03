use nari_gpu_d3d12 as gpu;
use nari_platform::{ControlFlow, Event, Platform};
use std::path::Path;
fn main() -> anyhow::Result<()> {
    let platform = Platform::new();

    unsafe {
        let instance = gpu::Instance::with_surface(&platform.surface)?;
        // let shader_path = Path::new("assets/shaders");
        // let mut gpu = gpu::Gpu::new(&instance, shader_path)?;

        let size = platform.surface.extent();
        // let mut wsi = gpu::Swapchain::new(
        //     &instance,
        //     &gpu,
        //     size.width as _,
        //     size.height as _,
        //     vk::PresentModeKHR::IMMEDIATE,
        // )?;

        platform.run(move |event_loop, event| {
            match event {
                Event::Resize(extent) => {
                    // wsi.resize(&gpu, extent.width as _, extent.height as _)
                    //     .unwrap();
                    event_loop.surface.redraw();
                }
                Event::Paint => {
                    // let size = event_loop.surface.extent();
                    // let frame = wsi.acquire().unwrap();
                    // let pool = gpu.acquire_pool().unwrap();

                    // gpu.cmd_barriers(
                    //     pool,
                    //     &[],
                    //     &[gpu::ImageBarrier {
                    //         image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                    //         src: gpu::ImageAccess {
                    //             access: gpu::Access::NONE,
                    //             stage: gpu::Stage::NONE,
                    //             layout: gpu::ImageLayout::UNDEFINED,
                    //         },
                    //         dst: gpu::ImageAccess {
                    //             access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                    //             stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    //             layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    //         },
                    //     }],
                    // );

                    // let area = vk::Rect2D {
                    //     offset: vk::Offset2D { x: 0, y: 0 },
                    //     extent: vk::Extent2D {
                    //         width: size.width as _,
                    //         height: size.height as _,
                    //     },
                    // };
                    // gpu.cmd_set_viewport(
                    //     pool.cmd_buffer,
                    //     0,
                    //     &[vk::Viewport {
                    //         x: 0.0,
                    //         y: 0.0,
                    //         width: size.width as _,
                    //         height: size.height as _,
                    //         min_depth: 0.0,
                    //         max_depth: 1.0,
                    //     }],
                    // );
                    // gpu.cmd_set_scissor(pool.cmd_buffer, 0, &[area]);
                    // let _t0 = gpu.cmd_timestamp(pool, gpu::Stage::TOP_OF_PIPE);
                    // gpu.cmd_graphics_begin(
                    //     pool,
                    //     area,
                    //     &[gpu::GraphicsAttachment {
                    //         image_view: wsi.frame_rtvs[frame.id],
                    //         layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    //         load: vk::AttachmentLoadOp::CLEAR,
                    //         store: vk::AttachmentStoreOp::STORE,
                    //         clear: vk::ClearValue {
                    //             color: vk::ClearColorValue { float32: [0.2; 4] },
                    //         },
                    //     }],
                    // );
                    // gpu.cmd_graphics_end(pool);

                    // gpu.cmd_barriers(
                    //     pool,
                    //     &[],
                    //     &[gpu::ImageBarrier {
                    //         image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                    //         src: gpu::ImageAccess {
                    //             access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                    //             stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    //             layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    //         },
                    //         dst: gpu::ImageAccess {
                    //             access: gpu::Access::NONE,
                    //             stage: gpu::Stage::NONE,
                    //             layout: gpu::ImageLayout::PRESENT_SRC_KHR,
                    //         },
                    //     }],
                    // );
                    // let _t1 = gpu.cmd_timestamp(pool, gpu::Stage::BOTTOM_OF_PIPE);

                    // let t_gpu = gpu
                    //     .submit_pool(
                    //         pool,
                    //         gpu::Submit {
                    //             waits: &[gpu::SemaphoreSubmit {
                    //                 semaphore: frame.acquire,
                    //                 stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    //             }],
                    //             signals: &[gpu::SemaphoreSubmit {
                    //                 semaphore: frame.present,
                    //                 stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                    //             }],
                    //         },
                    //     )
                    //     .unwrap();

                    // wsi.present(&gpu, frame).unwrap();
                    // if t_gpu > 3 {
                    //     let submission = gpu.wait(t_gpu - 3).unwrap();
                    //     for (_, ts) in submission.timestamps {
                    //         dbg!((ts[1] - ts[0]) * 1000.0); // ms
                    //     }
                    // }
                }
                Event::MouseMove { .. } => {
                    event_loop.surface.redraw();
                }
                _ => (),
            }

            ControlFlow::Continue
        });
    }

    Ok(())
}
