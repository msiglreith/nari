use nari_platform::{ControlFlow, Event, Platform, SurfaceArea};
use std::collections::HashMap;
use vello::{
    kurbo::{Affine, Point, Rect},
    peniko::{Brush, Color, Fill},
    Scene, SceneBuilder, SceneFragment,
};

fn render_text_run(
    sb: &mut SceneBuilder,
    text_run: &nari_vello::typo::TextRun,
    affine: Affine,
    align_x: nari_vello::Align,
    brush: Brush,
    glyph_cache: &nari_vello::typo::GlyphCache,
) {
    let px = text_run.offset_x(align_x);
    for cluster in &text_run.clusters {
        for glyph in &cluster.glyphs {
            let key = nari_vello::typo::GlyphKey {
                id: glyph.id,
                offset: glyph.offset.fract(),
            };
            let path = glyph_cache
                .get(&(text_run.font.size, key))
                .expect("missing glyph entry");
            let advance = px + glyph.offset.trunc().f64();

            sb.fill(
                Fill::NonZero,
                Affine::translate((advance as _, 0.0))
                    * affine
                    * Affine::scale_non_uniform(1.0, -1.0),
                &brush,
                None,
                &path,
            );
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let platform = Platform::new();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });
    let adapter = instance.request_adapter(&Default::default()).await.unwrap();
    let features = adapter.features();
    let limits = wgpu::Limits::default();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: features
                    & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::CLEAR_TEXTURE),
                limits,
            },
            None,
        )
        .await?;

    let mut renderer = vello::Renderer::new(&device).unwrap();
    let mut engine = nari_vello::Engine::new();
    let mut glyph_cache = nari_vello::typo::GlyphCache::default();

    let font = engine.create_font(std::fs::read("assets/segoeui.ttf")?);
    let mut font_table = HashMap::<nari_vello::typo::FontSize, _>::default();

    for ft in 6..25 {
        font_table.insert(ft, engine.create_font_scaled(font, ft));
    }

    let surface = unsafe { instance.create_surface(&platform.surface)? };
    let mut size = platform.surface.extent();
    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoNoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);

    let mut scene = Scene::default();

    let mut waterfall = SceneFragment::default();
    {
        let mut sb = SceneBuilder::for_fragment(&mut waterfall);
        let mut py = 0.0;

        for ft in 6..25 {
            let font = &font_table[&ft];
            let text_run = engine.build_text_run(
                *font,
                &format!("{}: The lazy dog 0123456789", ft),
                &mut glyph_cache,
            );
            render_text_run(
                &mut sb,
                &text_run,
                Affine::translate((0.0, py as _)),
                nari_vello::Align::Positive,
                vello::peniko::Brush::Solid(Color::rgb(1.0, 1.0, 1.0)),
                &glyph_cache,
            );
            py += font.properties.height;
        }
        sb.finish();
    }

    platform.run(move |event_loop, event| {
        match event {
            Event::Resize(extent) => {
                size = extent;
                surface_config.width = extent.width;
                surface_config.height = extent.height;
                surface.configure(&device, &surface_config);

                event_loop.surface.redraw();
            }

            Event::Hittest { x, y, area } => {
                const MARGIN: i32 = 5;
                const CAPTION_HEIGHT: i32 = 28;

                let w = size.width as i32;
                let h = size.height as i32;

                *area = match (x, y) {
                    (_, 0..=CAPTION_HEIGHT) => SurfaceArea::Caption,
                    _ => SurfaceArea::Client,
                };

                if !event_loop.surface.is_maximized() {
                    // resize border
                    *area = match (x, y) {
                        _ if x <= MARGIN && y <= MARGIN => SurfaceArea::TopLeft,
                        _ if x >= w - MARGIN && y <= MARGIN => SurfaceArea::TopRight,
                        _ if x >= w - MARGIN && y >= h - MARGIN => SurfaceArea::BottomRight,
                        _ if x <= MARGIN && y >= h - MARGIN => SurfaceArea::BottomLeft,
                        _ if x <= MARGIN => SurfaceArea::Left,
                        _ if y <= MARGIN => SurfaceArea::Top,
                        _ if x >= w - MARGIN => SurfaceArea::Right,
                        _ if y >= h - MARGIN => SurfaceArea::Bottom,
                        _ => *area,
                    };
                }
            }

            Event::Paint => {
                let t0 = std::time::Instant::now();
                superluminal_perf::begin_event("paint");
                let mut sb = SceneBuilder::for_scene(&mut scene);
                let rect = Rect::from_origin_size(
                    Point::new(0.0, 0.0),
                    (size.width as f64, size.height as f64),
                );
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.12, 0.14, 0.17)),
                    None,
                    &rect,
                );
                sb.append(&waterfall, Some(Affine::translate((20.0, 20.0))));

                sb.finish();
                // std::thread::sleep(std::time::Duration::from_millis(10));
                superluminal_perf::end_event();
                println!("{:?}", t0.elapsed());

                let frame_image = surface
                    .get_current_texture()
                    .expect("failed to get surface texture");

                renderer
                    .render_to_surface(
                        &device,
                        &queue,
                        &scene,
                        &frame_image,
                        size.width,
                        size.height,
                    )
                    .expect("failed to render to surface");

                frame_image.present();
                device.poll(wgpu::Maintain::Poll);
            }
            _ => (),
        }
        ControlFlow::Continue
    });

    Ok(())
}

fn main() -> anyhow::Result<()> {
    pollster::block_on(run())
}
