mod icon;

use nari_platform::{
    ControlFlow, Event, EventLoop, Extent, Key, KeyCode, KeyState, Modifiers, MouseButtons,
    Platform, SurfaceArea,
};
use nari_vello::{
    icon::Icon,
    kurbo::{Affine, Point, Rect, RoundedRect, Stroke},
    peniko::{Brush, Color, Fill},
    typo::{Caret, FontScaled},
    Align, Canvas, Scene, SceneBuilder,
};
use unicode_segmentation::GraphemeCursor;

// Pretty simple cursor, might not be ideal for more complex scripts
// with grapheme clusters.
struct TextCursor {
    text: String,
    pen: Point,
    cursor_pos: usize,
    focused: bool,
}

impl TextCursor {
    fn on_char(&mut self, c: char) {
        if !self.focused {
            return;
        }

        if c.is_control() {
            return;
        }

        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    fn on_key(&mut self, key: Key, state: KeyState, _modifiers: Modifiers) {
        if !self.focused {
            return;
        }

        const BACKSPACE: char = '\x08';

        match (key, state) {
            (Key::Char(BACKSPACE), KeyState::Down) => {
                // grapheme based, could be more sophisticated
                let mut cursor = GraphemeCursor::new(self.cursor_pos, self.text.len(), true);
                if let Some(pos) = cursor.prev_boundary(&self.text, 0).unwrap() {
                    let new_pos = pos;
                    for _ in new_pos..=pos {
                        self.text.remove(new_pos);
                    }
                    self.cursor_pos = new_pos;
                }
            }
            (Key::Code(KeyCode::Left), KeyState::Down) => {
                let mut cursor = GraphemeCursor::new(self.cursor_pos, self.text.len(), true);
                if let Some(pos) = cursor.prev_boundary(&self.text, 0).unwrap() {
                    self.cursor_pos = pos;
                }
            }
            (Key::Code(KeyCode::Right), KeyState::Down) => {
                let mut cursor = GraphemeCursor::new(self.cursor_pos, self.text.len(), true);
                if let Some(pos) = cursor.next_boundary(&self.text, 0).unwrap() {
                    self.cursor_pos = pos;
                }
            }
            _ => (),
        }
    }

    fn on_mouse(
        &mut self,
        app: &mut App,
        button: MouseButtons,
        state: KeyState,
        _modifiers: Modifiers,
    ) {
        match (button, state) {
            (MouseButtons::LEFT, KeyState::Down) => {
                let text_run = app
                    .canvas
                    .build_text_run(app.style.font_regular, &self.text);
                if let Some((x, y)) = app.event_loop.mouse_position {
                    let p = Point::new(x as _, y as _) - self.pen.to_vec2();
                    if let Some(Caret { cluster }) = text_run.hittest(p) {
                        self.focused = true;
                        self.cursor_pos = if cluster == text_run.clusters.len() {
                            self.text.len()
                        } else {
                            text_run.clusters[cluster].byte_pos
                        };
                    } else {
                        self.focused = false;
                    }
                }
            }
            _ => (),
        }
    }

    fn paint(&self, app: &mut App, mut sb: &mut SceneBuilder) {
        let pen = Affine::translate(self.pen.to_vec2());

        let text_run = app
            .canvas
            .build_text_run(app.style.font_regular, &self.text);
        let bounds = text_run.bounds();

        // selection background
        if self.focused {
            sb.fill(
                Fill::NonZero,
                pen,
                &Brush::Solid(app.style.color_text_select),
                None,
                &bounds,
            );
        }

        app.canvas.text_run(
            &mut sb,
            &text_run,
            pen,
            Align::Positive,
            Brush::Solid(app.style.color_text),
        );

        // draw caret
        if self.focused {
            let advance = text_run.cluster_advance(self.cursor_pos);
            sb.fill(
                Fill::NonZero,
                pen,
                &Brush::Solid(app.style.color_cursor),
                None,
                &Rect {
                    x0: advance,
                    x1: advance + 1.0,
                    ..bounds
                },
            );
        }
    }
}

struct Border;
impl Border {
    const MARGIN: f64 = 5.0;
    fn hittest(app: &App, p: Point) -> Option<SurfaceArea> {
        if app.event_loop.surface.is_maximized() {
            return None;
        }

        let Extent { width, height } = app.event_loop.surface.extent();

        if p.x <= Self::MARGIN {
            return if p.y <= Self::MARGIN {
                Some(SurfaceArea::TopLeft)
            } else if p.y >= height - Self::MARGIN {
                Some(SurfaceArea::BottomLeft)
            } else {
                Some(SurfaceArea::Left)
            };
        }

        if p.x >= width - Self::MARGIN {
            return if p.y <= Self::MARGIN {
                Some(SurfaceArea::TopRight)
            } else if p.y >= height - Self::MARGIN {
                Some(SurfaceArea::BottomRight)
            } else {
                Some(SurfaceArea::Right)
            };
        }

        if p.y <= Self::MARGIN {
            return Some(SurfaceArea::Top);
        }
        if p.y >= height - Self::MARGIN {
            return Some(SurfaceArea::Bottom);
        }

        None
    }
}

struct Caption;
impl Caption {
    const BUTTON_WIDTH: f64 = 46.0;
    const BUTTON_HEIGHT: f64 = 28.0;
    const CAPTION_HEIGHT: f64 = Self::BUTTON_HEIGHT;

    fn button_minimize(canvas: &Canvas, extent: Extent) -> Rect {
        Rect {
            x0: extent.width - 3.0 * canvas.scale(Self::BUTTON_WIDTH),
            x1: extent.width - 2.0 * canvas.scale(Self::BUTTON_WIDTH),
            y0: 0.0,
            y1: canvas.scale(Self::BUTTON_HEIGHT),
        }
    }

    fn button_maximize(canvas: &Canvas, extent: Extent) -> Rect {
        Rect {
            x0: extent.width - 2.0 * canvas.scale(Self::BUTTON_WIDTH),
            x1: extent.width - canvas.scale(Self::BUTTON_WIDTH),
            y0: 0.0,
            y1: canvas.scale(Self::BUTTON_HEIGHT),
        }
    }

    fn button_close(canvas: &Canvas, extent: Extent) -> Rect {
        Rect {
            x0: extent.width - canvas.scale(Self::BUTTON_WIDTH),
            x1: extent.width,
            y0: 0.0,
            y1: canvas.scale(Self::BUTTON_HEIGHT),
        }
    }

    fn hittest(app: &App, p: Point) -> Option<SurfaceArea> {
        let extent = app.event_loop.surface.extent();
        let canvas = &app.canvas;

        let chrome_minimize = Self::button_minimize(canvas, extent);
        if chrome_minimize.contains(p) {
            return Some(SurfaceArea::Minimize);
        }

        let chrome_maximize = Self::button_maximize(canvas, extent);
        if chrome_maximize.contains(p) {
            return Some(SurfaceArea::Maximize);
        }

        let chrome_close = Self::button_close(canvas, extent);
        if chrome_close.contains(p) {
            return Some(SurfaceArea::Close);
        }

        if p.y <= canvas.scale(Self::CAPTION_HEIGHT) {
            return Some(SurfaceArea::Caption);
        }

        None
    }

    fn paint(app: &mut App, mut sb: &mut SceneBuilder) {
        let extent = app.event_loop.surface.extent();
        let canvas = &app.canvas;

        let affine_dpi = Affine::scale(canvas.scale(1.0));

        let chrome_minimize = Self::button_minimize(canvas, extent);
        let chrome_maximize = Self::button_maximize(canvas, extent);
        let chrome_close = Self::button_close(canvas, extent);

        // hover background
        let mouse_pos = app.event_loop.mouse_position.map(|(x, y)| Point::new(x as f64, y as f64));
        if let Some(p) = mouse_pos {
            if chrome_minimize.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.85, 0.85, 0.85)),
                    None,
                    &chrome_minimize,
                );
            } else if chrome_maximize.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.85, 0.85, 0.85)),
                    None,
                    &chrome_maximize,
                );
            } else if chrome_close.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.80, 0.20, 0.15)),
                    None,
                    &chrome_close,
                );
            }
        }



        let affine_minimize = Affine::translate(
            (chrome_minimize.center() - canvas.scale_pt(app.style.icon_chrome_minimize.bbox.center())).floor(),
        );
        app.style.icon_chrome_minimize.paint(
            &mut sb,
            affine_minimize * affine_dpi,
            &Brush::Solid(app.style.color_text),
        );

        let icon_maximize = if app.event_loop.surface.is_maximized() {
            &app.style.icon_chrome_restore
        } else {
            &app.style.icon_chrome_maximize
        };
        let affine_maximize =
            Affine::translate((chrome_maximize.center() - canvas.scale_pt(icon_maximize.bbox.center())).floor());
        icon_maximize.paint(
            &mut sb,
            affine_maximize * affine_dpi,
            &Brush::Solid(app.style.color_text),
        );

        let close_color = if let Some(p) = mouse_pos {
            if chrome_close.contains(p) {
                Color::rgb(1.0, 1.0, 1.0)
            } else {
                app.style.color_text
            }
        } else {
            app.style.color_text
        };
        let affine_close = Affine::translate(
            (chrome_close.center() - canvas.scale_pt(app.style.icon_chrome_close.bbox.center())).floor(),
        );
        app.style.icon_chrome_close.paint(
            &mut sb,
            affine_close * affine_dpi,
            &Brush::Solid(close_color),
        );
    }
}

struct Style {
    font_regular: FontScaled,

    logo: Icon,

    icon_chrome_minimize: Icon,
    icon_chrome_maximize: Icon,
    icon_chrome_restore: Icon,
    icon_chrome_close: Icon,

    color_caption: Color,
    color_text: Color,
    color_background: Color,
    color_text_select: Color,
    color_cursor: Color,
}

struct App {
    canvas: Canvas,
    event_loop: EventLoop,
    style: Style,
}

async fn run() -> anyhow::Result<()> {
    let platform = Platform::new();

    let mut canvas = Canvas::new(platform.surface).await;

    let icon_chrome_close = Icon::build(&std::fs::read("assets/codicon/chrome-close.svg")?)?;
    let icon_chrome_minimize = Icon::build(&std::fs::read("assets/codicon/chrome-minimize.svg")?)?;
    let icon_chrome_maximize = Icon::build(&std::fs::read("assets/codicon/chrome-maximize.svg")?)?;
    let icon_chrome_restore = Icon::build(&std::fs::read("assets/codicon/chrome-restore.svg")?)?;

    let logo = Icon::build(&std::fs::read("assets/logo.svg")?)?;

    let font_body = canvas.create_font(std::fs::read("assets/Inter/Inter-Regular.ttf")?);
    let font_body_regular = canvas.create_font_scaled(font_body, canvas.scale(16.0).round() as u32);

    let mut text_cursor = TextCursor {
        pen: canvas.scale_pt(Point::new(10.0, 50.0)),
        cursor_pos: 0,
        text: "hello world! ".to_string(),
        focused: false,
    };

    let mut text_cursor2 = TextCursor {
        pen: canvas.scale_pt(Point::new(10.0, 70.0)),
        cursor_pos: 0,
        text: "test row 2".to_string(),
        focused: false,
    };

    let mut app = App {
        canvas,
        event_loop: EventLoop {
            surface: platform.surface,
            mouse_position: None,
            mouse_buttons: MouseButtons::empty(),
        },
        style: Style {
            font_regular: font_body_regular,

            logo,

            icon_chrome_close,
            icon_chrome_minimize,
            icon_chrome_maximize,
            icon_chrome_restore,

            color_background: Color::rgb(1.0, 1.0, 1.0),
            color_caption: Color::rgb(0.97, 0.97, 1.0),
            color_text: Color::rgb(0.0, 0.0, 0.0),
            color_text_select: Color::rgb(0.97, 0.97, 1.0),
            color_cursor: Color::rgb(0.0, 0.0, 0.0),
        },
    };

    let mut scene = Scene::default();

    platform.run(move |event_loop, event| {
        app.event_loop = event_loop;

        match event {
            Event::Resize(extent) => {
                app.canvas.resize(extent);
                app.event_loop.surface.redraw();
            }

            Event::Hittest { x, y, area } => {
                let p = Point::new(x as f64, y as f64);

                if let Some(hit_area) = Border::hittest(&app, p) {
                    *area = hit_area;
                } else if let Some(hit_area) = Caption::hittest(&app, p) {
                    *area = hit_area;
                }
            }

            Event::Paint => {
                let mut sb = SceneBuilder::for_scene(&mut scene);

                Caption::paint(&mut app, &mut sb);

                text_cursor.paint(&mut app, &mut sb);
                text_cursor2.paint(&mut app, &mut sb);

                let pen = Affine::translate((app.canvas.scale(30.0), app.canvas.scale(400.0)));

                let text_run = app
                    .canvas
                    .build_text_run(app.style.font_regular, "New Task");
                let bounds = text_run.bounds().inflate(app.canvas.scale(10.0), app.canvas.scale(5.0));
                let bounds_round = RoundedRect::from_rect(bounds, app.canvas.scale(6.0));

                sb.fill(
                    Fill::NonZero,
                    pen,
                    &Brush::Solid(app.style.color_text_select),
                    None,
                    &bounds_round,
                );
                sb.stroke(
                    &Stroke::new(1.0),
                    pen,
                    &Brush::Solid(Color::rgb(0.42, 0.45, 0.47)),
                    None,
                    &bounds_round,
                );

                app.canvas.text_run(
                    &mut sb,
                    &text_run,
                    pen,
                    Align::Positive,
                    Brush::Solid(app.style.color_text),
                );

                app.canvas.present(&scene, app.style.color_background);
            }

            Event::Key {
                key,
                state,
                modifiers,
            } => {
                text_cursor.on_key(key, state, modifiers);
                text_cursor2.on_key(key, state, modifiers);

                app.event_loop.surface.redraw();
            }

            Event::MouseButton {
                button,
                state,
                modifiers,
            } => {
                text_cursor.on_mouse(&mut app, button, state, modifiers);
                text_cursor2.on_mouse(&mut app, button, state, modifiers);

                app.event_loop.surface.redraw();
            }

            Event::Char(c) => {
                text_cursor.on_char(c);
                text_cursor2.on_char(c);

                app.event_loop.surface.redraw();
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
