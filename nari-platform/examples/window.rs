use nari_canvas as canvas;
use nari_gpu as gpu;
use nari_platform::{ControlFlow, Event, Extent, Platform, SurfaceArea};
use std::collections::VecDeque;

const MARGIN: i32 = 5;
const CURSOR: canvas::Color = [0.0, 0.95, 1.0, 1.0];
const TEXT_DEFAULT: canvas::Color = [1.0, 1.0, 1.0, 1.0];
const TEXT_SELECT: canvas::Color = [0.15, 0.17, 0.22, 1.0];
const BACKGROUND: canvas::Color = [0.12, 0.14, 0.17, 1.0];

const CAPTION_HEIGHT: i32 = 29;
const CLOSE_WIDTH: u32 = 45;
struct Ui {
    extent: Extent,
    wsi: gpu::Swapchain,
    timeline: VecDeque<gpu::Timestamp>,
    canvas: canvas::Canvas,

    // pool for encoding canvas elements.
    // only valid between `begin_frame` and `end_frame`.
    pool: gpu::Pool,
}

impl Ui {
    // Resize swapchain and dependent resources (ie canvas internal rendertarget).
    unsafe fn resize(&mut self, size: Extent) {
        self.extent = size;
        self.wsi
            .resize(&self.canvas, size.width, size.height)
            .unwrap();
        self.canvas.resize(size.width, size.height);
    }

    unsafe fn begin_frame(&mut self) -> gpu::Frame {
        let frame = self.wsi.acquire().unwrap();
        let t_wait = self.timeline.pop_front().expect("no pending frames");
        self.canvas.wait(t_wait).unwrap();

        self.pool = self.canvas.acquire_pool().unwrap();
        self.canvas.cmd_barriers(
            self.pool,
            &[],
            &[gpu::ImageBarrier {
                image: self.wsi.frame_images[frame.id].aspect(gpu::vk::ImageAspectFlags::COLOR),
                src: gpu::ImageAccess::UNDEFINED,
                dst: gpu::ImageAccess::COLOR_ATTACHMENT_WRITE,
            }],
        );

        self.canvas.composition_begin(self.pool);

        frame
    }

    unsafe fn end_frame(&mut self, frame: gpu::Frame) {
        let area = gpu::Rect2D {
            offset: gpu::Offset2D { x: 0, y: 0 },
            extent: gpu::Extent2D {
                width: self.extent.width,
                height: self.extent.height,
            },
        };

        self.canvas.composition_end(
            area,
            self.wsi.frame_rtvs[frame.id],
            gpu::vk::AttachmentLoadOp::CLEAR,
            BACKGROUND,
        );

        self.canvas.cmd_barriers(
            self.pool,
            &[],
            &[gpu::ImageBarrier {
                image: self.wsi.frame_images[frame.id].aspect(gpu::vk::ImageAspectFlags::COLOR),
                src: gpu::ImageAccess::COLOR_ATTACHMENT_WRITE,
                dst: gpu::ImageAccess::PRESENT,
            }],
        );

        let timestamp = self
            .canvas
            .submit_pool(
                self.pool,
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

        self.timeline.push_back(timestamp);
        self.wsi.present(&self.canvas, frame);
        self.pool = gpu::Pool::null();
    }
}

fn main() -> anyhow::Result<()> {
    unsafe {
        let platform = Platform::new();
        let mut size = platform.surface.extent();

        let instance = gpu::Instance::new(&platform.surface)?;
        let mut gpu = gpu::Gpu::new(&instance, std::path::Path::new("assets/shaders"))?;

        dbg!(size);
        let mut wsi = gpu::Swapchain::new(
            &instance,
            &gpu,
            size.width,
            size.height,
            gpu::vk::PresentModeKHR::IMMEDIATE,
        )?;

        let canvas = canvas::Canvas::new(
            gpu,
            size.width,
            size.height,
            wsi.swapchain_desc.image_format,
        );

        let mut ui = Ui {
            extent: size,
            wsi,
            canvas,
            timeline: VecDeque::from([0; 2]),
            pool: gpu::Pool::null(),
        };

        platform.run(move |event_loop, event| {
            match event {
                Event::Resize(extent) => {
                    size = extent;
                    ui.resize(size);
                }
                Event::Hittest { x, y, area } => {
                    let w = size.width as i32;
                    let h = size.height as i32;

                    *area = match (x, y) {
                        _ if x <= MARGIN && y <= MARGIN => SurfaceArea::TopLeft,
                        _ if x >= w - MARGIN && y <= MARGIN => SurfaceArea::TopRight,
                        _ if x >= w - MARGIN && y >= h - MARGIN => SurfaceArea::BottomRight,
                        _ if x <= MARGIN && y >= h - MARGIN => SurfaceArea::BottomLeft,
                        _ if x <= MARGIN => SurfaceArea::Left,
                        _ if y <= MARGIN => SurfaceArea::Top,
                        _ if x >= w - MARGIN => SurfaceArea::Right,
                        _ if y >= h - MARGIN => SurfaceArea::Bottom,
                        (_, 0..=CAPTION_HEIGHT) => SurfaceArea::Caption,
                        _ => SurfaceArea::Client,
                    };
                }
                Event::Paint => {
                    let frame = ui.begin_frame();

                    let chrome_minimize = canvas::Rect {
                        x0: size.width.saturating_sub(3 * CLOSE_WIDTH) as _,
                        x1: size.width.saturating_sub(2 * CLOSE_WIDTH) as _,
                        y0: 0,
                        y1: CAPTION_HEIGHT as _,
                    };
                    let chrome_maximize = canvas::Rect {
                        x0: size.width.saturating_sub(2 * CLOSE_WIDTH) as _,
                        x1: size.width.saturating_sub(CLOSE_WIDTH) as _,
                        y0: 0,
                        y1: CAPTION_HEIGHT as _,
                    };
                    let chrome_close = canvas::Rect {
                        x0: size.width.saturating_sub(CLOSE_WIDTH) as _,
                        x1: size.width as _,
                        y0: 0,
                        y1: CAPTION_HEIGHT as _,
                    };

                    if let Some((x, y)) = event_loop.mouse_position {
                        if chrome_minimize.hittest(x, y) {
                            ui.canvas.rect(chrome_minimize, [0.27, 0.3, 0.34, 1.0]);
                        } else if chrome_maximize.hittest(x, y) {
                            ui.canvas.rect(chrome_maximize, [0.27, 0.3, 0.34, 1.0]);
                        } else if chrome_close.hittest(x, y) {
                            ui.canvas.rect(chrome_close, [0.9, 0.07, 0.14, 1.0]);
                        }
                    }
                    ui.end_frame(frame);
                }
            }

            ControlFlow::Continue
        });

        Ok(())
    }
}
