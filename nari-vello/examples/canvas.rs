use nari_platform::{ControlFlow, Event, Platform, SurfaceArea};
use nari_vello::Canvas;
use std::collections::HashMap;
use vello::{
    kurbo::{Affine, Point, Rect},
    peniko::{Brush, Color, Fill},
    Scene, SceneBuilder, SceneFragment,
};

async fn run() -> anyhow::Result<()> {
    let platform = Platform::new();

    let mut canvas = Canvas::new(platform.surface).await;

    let font = canvas.create_font(std::fs::read("assets/segoeui.ttf")?);
    let mut font_table = HashMap::<nari_vello::typo::FontSize, _>::default();

    for ft in 6..25 {
        font_table.insert(ft, canvas.create_font_scaled(font, ft));
    }

    let mut scene = Scene::default();
    let mut waterfall = SceneFragment::default();
    {
        let mut sb = SceneBuilder::for_fragment(&mut waterfall);
        let mut py = 0.0;

        for ft in 6..25 {
            let font = &font_table[&ft];
            let text_run =
                canvas.build_text_run(*font, &format!("{}: The lazy dog 0123456789", ft));
            canvas.text_run(
                &mut sb,
                &text_run,
                Affine::translate((0.0, py as _)),
                nari_vello::Align::Positive,
                vello::peniko::Brush::Solid(Color::rgb(1.0, 1.0, 1.0)),
            );
            py += font.properties.height;
        }
        sb.finish();
    }

    let mut size = platform.surface.extent();
    platform.run(move |event_loop, event| {
        match event {
            Event::Resize(extent) => {
                size = extent;
                canvas.resize(extent);
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
                superluminal_perf::end_event();
                println!("{:?}", t0.elapsed());

                canvas.present(&scene);
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
