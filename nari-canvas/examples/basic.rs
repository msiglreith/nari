use gpu::vk;
use nari_canvas as canvas;
use nari_gpu as gpu;
use std::collections::HashMap;
use std::path::Path;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use zeno::{apply, Cap, Command, Fill, Join, Point, Stroke, Style, Transform};

struct PathCommand<'a> {
    style: Style<'a>,
    transform: Option<Transform>,
}

fn encode_node<F>(node: &usvg::Node, encode: &mut F)
where
    F: FnMut(PathCommand, &[Command]),
{
    match *node.borrow() {
        usvg::NodeKind::Path(ref p) => {
            let mut path = Vec::<Command>::new();
            for segment in p.data.0.iter() {
                match *segment {
                    usvg::PathSegment::MoveTo { x, y } => {
                        path.push(Command::MoveTo(Point::new(x as f32, y as f32)));
                    }
                    usvg::PathSegment::LineTo { x, y } => {
                        path.push(Command::LineTo(Point::new(x as f32, y as f32)));
                    }
                    usvg::PathSegment::CurveTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        path.push(Command::CurveTo(
                            Point::new(x1 as f32, y1 as f32),
                            Point::new(x2 as f32, y2 as f32),
                            Point::new(x as f32, y as f32),
                        ));
                    }
                    usvg::PathSegment::ClosePath => {
                        path.push(Command::Close);
                    }
                }
            }

            if let Some(_) = p.fill {
                (encode)(
                    PathCommand {
                        style: Style::Fill(Fill::EvenOdd),
                        transform: None,
                    },
                    &path,
                );
            }

            if let Some(ref s) = p.stroke {
                (encode)(
                    PathCommand {
                        style: Style::Stroke(Stroke {
                            width: s.width.value() as _,
                            join: Join::Round,
                            miter_limit: s.miterlimit.value() as _,
                            start_cap: Cap::Round,
                            end_cap: Cap::Round,
                            dashes: &[],
                            offset: 0.0,
                            scale: true,
                        }),
                        transform: None,
                    },
                    &path,
                );
            }
        }
        _ => {}
    }

    for child in node.children() {
        encode_node(&child, encode);
    }
}

fn main() -> anyhow::Result<()> {
    let args = &std::env::args().collect::<Vec<_>>();
    let file_path = &args[1];
    let svg_data = std::fs::read_to_string(file_path).unwrap();
    let svg = usvg::Tree::from_str(&svg_data, &usvg::Options::default().to_ref()).unwrap();

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("nari-canvas :: basic")
        .with_inner_size(LogicalSize::new(1440.0f32, 800.0))
        .build(&event_loop)?;

    unsafe {
        let instance = gpu::Instance::new(&window)?;
        let gpu = gpu::Gpu::new(&instance, Path::new("assets/shaders"))?;

        let mut submit_timestamps = [0; 2];
        let mut submit_id = 0;

        let mut size = window.inner_size();
        let mut wsi = gpu::Swapchain::new(
            &instance,
            &gpu,
            size.width,
            size.height,
            vk::PresentModeKHR::IMMEDIATE,
        )?;

        let mut canvas = canvas::Canvas::new(
            gpu,
            size.width,
            size.height,
            wsi.swapchain_desc.image_format,
        );

        let codicon = canvas.create_font(std::fs::read("assets/codicon.ttf")?);
        let codicon = canvas.create_font_scaled(codicon, 16);

        let fira_code = canvas.create_font(std::fs::read("assets/segoeui.ttf")?);
        let mut fira_code_table = HashMap::<canvas::typo::FontSize, _>::default();

        for ft in 6..25 {
            fira_code_table.insert(ft, canvas.create_font_scaled(fira_code, ft));
        }

        let upload_pool = canvas.acquire_pool().unwrap();
        let mut layers = Vec::new();
        encode_node(&svg.root(), &mut |cmd, path| {
            let mut path_flat = Vec::<Command>::new();
            apply(path, cmd.style, cmd.transform, &mut path_flat);
            let tiles = canvas.build_path(upload_pool, &path_flat);
            layers.push(tiles);
        });

        canvas
            .submit_pool(
                upload_pool,
                gpu::Submit {
                    waits: &[],
                    signals: &[],
                },
            )
            .unwrap();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(_) => {
                        size = window.inner_size();
                        wsi.resize(&canvas, size.width, size.height).unwrap();
                        canvas.resize(size.width, size.height);
                    }
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => (),
                },
                Event::MainEventsCleared => window.request_redraw(),
                Event::RedrawRequested(_) => {
                    let area = gpu::Rect2D {
                        offset: gpu::Offset2D { x: 0, y: 0 },
                        extent: gpu::Extent2D {
                            width: size.width,
                            height: size.height,
                        },
                    };

                    let frame = wsi.acquire().unwrap();
                    let local_id = submit_id % submit_timestamps.len();
                    let t_wait = submit_timestamps[local_id];
                    canvas.wait(t_wait).unwrap();

                    let pool = canvas.acquire_pool().unwrap();

                    canvas.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess::UNDEFINED,
                            dst: gpu::ImageAccess::COLOR_ATTACHMENT_WRITE,
                        }],
                    );

                    let mut pen = canvas::typo::Pen::default();

                    let background = [0.0, 0.0, 0.0, 1.0]; // [0.12, 0.13, 0.15, 1.0];
                    let foreground = [1.0, 1.0, 1.0, 1.0];

                    // for layer in &layers {
                    //     canvas.path(layer, 0, 0, background);
                    // }

                    canvas.composition_begin(pool);

                    // Gradients
                    canvas.hrect(
                        canvas::Rect {
                            x0: 400,
                            x1: 800,
                            y0: 420,
                            y1: 450,
                        },
                        [0.0, 0.0, 0.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    canvas.hrect(
                        canvas::Rect {
                            x0: 400,
                            x1: 800,
                            y0: 450,
                            y1: 480,
                        },
                        [1.0, 1.0, 1.0, 1.0],
                        [0.0, 0.0, 0.0, 1.0],
                    );
                    canvas.hrect(
                        canvas::Rect {
                            x0: 400,
                            x1: 800,
                            y0: 480,
                            y1: 510,
                        },
                        [1.0, 0.0, 0.0, 1.0],
                        [0.0, 0.0, 1.0, 1.0],
                    );

                    // Text waterfall

                    pen.x = 20;
                    pen.y = 20;
                    pen.color = background;

                    for ft in 6..25 {
                        let font = &fira_code_table[&ft];
                        canvas.text(
                            fira_code_table[&ft],
                            pen,
                            &format!("{}: The lazy dog 0123456789", ft),
                        );
                        pen.y += font.properties.height as i32;
                    }

                    canvas.rect(
                        canvas::Rect {
                            x0: 400,
                            x1: 800,
                            y0: 0,
                            y1: 400,
                        },
                        [0.0, 0.0, 0.0, 1.0],
                    );
                    pen.x = 420;
                    pen.y = 20;
                    pen.color = foreground;

                    for ft in 6..25 {
                        let font = &fira_code_table[&ft];
                        canvas.text(
                            fira_code_table[&ft],
                            pen,
                            &format!("{}: The lazy dog 0123456789", ft),
                        );
                        pen.y += font.properties.height;
                    }

                    canvas.rect(
                        canvas::Rect {
                            x0: 800,
                            x1: 1200,
                            y0: 0,
                            y1: 400,
                        },
                        [0.0, 1.0, 0.0, 1.0],
                    );
                    pen.x = 820;
                    pen.y = 20;
                    pen.color = [1.0, 0.0, 0.0, 1.0];

                    for ft in 6..25 {
                        let font = &fira_code_table[&ft];
                        canvas.text(
                            fira_code_table[&ft],
                            pen,
                            &format!("{}: The lazy dog 0123456789", ft),
                        );
                        pen.y += font.properties.height;
                    }

                    let chrome_close = canvas::Rect {
                        x0: 1440 - 45,
                        x1: 1440,
                        y0: 0,
                        y1: 29,
                    };
                    canvas.rect(chrome_close, [0.0, 0.0, 0.0, 1.0]);

                    canvas.text(
                        codicon,
                        canvas::typo::Pen {
                            x: (chrome_close.x0 + chrome_close.x1) / 2,
                            y: (chrome_close.y0 + chrome_close.y1) / 2 + 8,
                            color: foreground,
                            align_x: canvas::Align::Center,
                        },
                        canvas::Codicon::ChromeClose,
                    );

                    canvas.squircle(
                        canvas::Squircle {
                            rect: canvas::Rect {
                                x0: 50,
                                x1: 250,
                                y0: 400,
                                y1: 540,
                            },
                            radius: 50,
                            smoothing: 0.6,
                        },
                        background,
                    );

                    canvas.composition_end(
                        area,
                        wsi.frame_rtvs[frame.id],
                        vk::AttachmentLoadOp::CLEAR,
                        foreground,
                    );

                    canvas.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess::COLOR_ATTACHMENT_WRITE,
                            dst: gpu::ImageAccess::PRESENT,
                        }],
                    );

                    let render = canvas
                        .submit_pool(
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

                    submit_timestamps[local_id] = render;
                    submit_id += 1;

                    wsi.present(&canvas, frame).unwrap();
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
