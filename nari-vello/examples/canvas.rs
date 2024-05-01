use nari_platform::{ControlFlow, Event, Platform, SurfaceArea};
use nari_vello::{
    kurbo::{Affine, Point, Rect},
    peniko::{Brush, Color, Fill},
    Canvas, Scene,
};
use std::collections::HashMap;

async fn run() -> anyhow::Result<()> {
    let background: Color = Color::rgb(1.0, 1.0, 1.0);
    let foreground: Color = Color::rgb(0.0, 0.0, 0.0);

    let platform = Platform::new();

    let mut canvas = Canvas::new(platform.surface).await;
    let decor = nari_decor_basic::Theme::new()?;

    let font = canvas.create_font(std::fs::read("assets/Inter/Inter-Regular.ttf")?);
    let mut font_table = HashMap::<nari_vello::typo::FontSize, _>::default();

    for ft in 6..25 {
        font_table.insert(
            ft,
            canvas.create_font_scaled(font, canvas.scale(ft as f64) as _),
        );
    }

    let mut waterfall = Scene::default();
    {
        let mut sb = &mut waterfall;
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
                vello::peniko::Brush::Solid(foreground),
            );
            py += font.properties.height.round();
        }
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
                if let Some(hit) = decor.hit_test(&event_loop, x, y) {
                    *area = hit;
                } else {
                    *area = SurfaceArea::Client;
                };
            }

            Event::Paint => {
                let t0 = std::time::Instant::now();
                superluminal_perf::begin_event("paint");

                let mut scene = Scene::default();
                let mut sb = &mut scene;
                let rect = Rect::from_origin_size(
                    Point::new(0.0, 0.0),
                    (size.width as f64, size.height as f64),
                );
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(background),
                    None,
                    &rect,
                );
                sb.append(&waterfall, Some(Affine::translate((20.0, 20.0))));

                decor.paint(&event_loop, &mut canvas, &mut sb);

                superluminal_perf::end_event();
                println!("{:?}", t0.elapsed());

                canvas.present(&scene, Color::WHITE);
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
