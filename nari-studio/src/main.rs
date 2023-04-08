//         // rendering
//         let text_run = ui.canvas.build_text_run(ui.pool, self.font, &self.text);
//         if self.focused {
//             let bounds = text_run.bounds(self.pen);
//             let cursor_advance = self.pen.x + text_run.cluster_advance(self.pos);

//             ui.canvas.rect(text_run.bounds(self.pen), TEXT_SELECT);
//             ui.canvas.rect(
//                 canvas::Rect {
//                     x0: cursor_advance,
//                     x1: cursor_advance + 1,
//                     ..bounds
//                 },
//                 CURSOR,
//             );
//         }
//         ui.canvas.text_run(self.pen, &text_run);
//     }
// }

use nari_platform::{
    ControlFlow, Event, EventLoop, Extent, Key, KeyCode, KeyState, Modifiers, MouseButtons,
    Platform, Surface, SurfaceArea,
};
use nari_vello::{
    kurbo::{Affine, Point, Rect},
    peniko::{Brush, Color, Fill},
    typo::{Caret, Font, FontScaled},
    Align, Canvas, Codicon, Scene, SceneBuilder, SceneFragment,
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

        const BACKSPACE: char = '\x0b';

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
                let text_run = app.canvas.build_text_run(app.font_body_regular, &self.text);
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
        let text_run = app.canvas.build_text_run(app.font_body_regular, &self.text);
        app.canvas.text_run(
            &mut sb,
            &text_run,
            Affine::translate(self.pen.to_vec2()),
            Align::Positive,
            Brush::Solid(app.foreground),
        );
    }
}

struct App {
    canvas: Canvas,
    event_loop: EventLoop,

    codicon: Font,
    codicon_regular: FontScaled,
    font_body: Font,
    font_body_regular: FontScaled,

    background: Color,
    foreground: Color,
}

struct Border;
impl Border {
    const MARGIN: f64 = 5.0;
    fn hittest(app: &App, p: Point) -> Option<SurfaceArea> {
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

    fn button_minimize(extent: Extent) -> Rect {
        Rect {
            x0: extent.width - 3.0 * Self::BUTTON_WIDTH,
            x1: extent.width - 2.0 * Self::BUTTON_WIDTH,
            y0: 0.0,
            y1: Self::BUTTON_HEIGHT,
        }
    }

    fn button_maximize(extent: Extent) -> Rect {
        Rect {
            x0: extent.width - 2.0 * Self::BUTTON_WIDTH,
            x1: extent.width - Self::BUTTON_WIDTH,
            y0: 0.0,
            y1: Self::BUTTON_HEIGHT,
        }
    }

    fn button_close(extent: Extent) -> Rect {
        Rect {
            x0: extent.width - Self::BUTTON_WIDTH,
            x1: extent.width,
            y0: 0.0,
            y1: Self::BUTTON_HEIGHT,
        }
    }

    fn hittest(app: &App, p: Point) -> Option<SurfaceArea> {
        let extent = app.event_loop.surface.extent();

        let chrome_minimize = Self::button_minimize(extent);
        if chrome_minimize.contains(p) {
            return Some(SurfaceArea::Minimize);
        }

        let chrome_maximize = Self::button_maximize(extent);
        if chrome_maximize.contains(p) {
            return Some(SurfaceArea::Maximize);
        }

        let chrome_close = Self::button_close(extent);
        if chrome_close.contains(p) {
            return Some(SurfaceArea::Close);
        }

        if p.y <= Self::CAPTION_HEIGHT {
            return Some(SurfaceArea::Caption);
        }

        None
    }

    fn paint(app: &mut App, mut sb: &mut SceneBuilder) {
        let extent = app.event_loop.surface.extent();

        let chrome_minimize = Self::button_minimize(extent);
        let chrome_maximize = Self::button_maximize(extent);
        let chrome_close = Self::button_close(extent);

        // hover background
        if let Some((x, y)) = app.event_loop.mouse_position {
            let p = Point::new(x as f64, y as f64);

            if chrome_minimize.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.27, 0.3, 0.34)),
                    None,
                    &chrome_minimize,
                );
            } else if chrome_maximize.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.27, 0.3, 0.34)),
                    None,
                    &chrome_maximize,
                );
            } else if chrome_close.contains(p) {
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.9, 0.07, 0.14)),
                    None,
                    &chrome_close,
                );
            }
        }

        // show symbols
        let affine_minimize = Affine::translate(
            chrome_minimize.center()
                - app
                    .canvas
                    .glyph_extent(app.codicon_regular, Codicon::ChromeMinimize)
                    .center(),
        );
        app.canvas.glyph(
            &mut sb,
            app.codicon_regular,
            Codicon::ChromeMinimize,
            affine_minimize,
            &Brush::Solid(app.foreground),
        );

        let affine_maximize = Affine::translate(
            chrome_maximize.center()
                - app
                    .canvas
                    .glyph_extent(app.codicon_regular, Codicon::ChromeMaximize)
                    .center(),
        );
        app.canvas.glyph(
            &mut sb,
            app.codicon_regular,
            Codicon::ChromeMaximize,
            affine_maximize,
            &Brush::Solid(app.foreground),
        );

        let affine_close = Affine::translate(
            chrome_close.center()
                - app
                    .canvas
                    .glyph_extent(app.codicon_regular, Codicon::ChromeClose)
                    .center(),
        );
        app.canvas.glyph(
            &mut sb,
            app.codicon_regular,
            Codicon::ChromeClose,
            affine_close,
            &Brush::Solid(app.foreground),
        );
    }
}

async fn run() -> anyhow::Result<()> {
    let platform = Platform::new();

    let mut canvas = Canvas::new(platform.surface).await;

    let font_body = canvas.create_font(std::fs::read("assets/Inter/Inter-Regular.ttf")?);
    let font_body_regular = canvas.create_font_scaled(font_body, 16);
    let codicon = canvas.create_font(std::fs::read("assets/codicon/codicon.ttf")?);
    let codicon_regular = canvas.create_font_scaled(codicon, 16);

    let background: Color = Color::rgb(0.12, 0.14, 0.17);
    let foreground: Color = Color::rgb(1.0, 1.0, 1.0);

    let mut app = App {
        canvas,
        event_loop: EventLoop {
            surface: platform.surface,
            mouse_position: None,
            mouse_buttons: MouseButtons::empty(),
        },
        codicon,
        codicon_regular,
        font_body,
        font_body_regular,
        background,
        foreground,
    };

    let mut text_cursor = TextCursor {
        pen: Point::new(10.0, 30.0),
        cursor_pos: 0,
        text: "hello world! ".to_string(),
        focused: false,
    };

    let mut text_cursor2 = TextCursor {
        pen: Point::new(10.0, 50.0),
        cursor_pos: 0,
        text: "test row 2".to_string(),
        focused: false,
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
                let size = app.event_loop.surface.extent();
                let mut sb = SceneBuilder::for_scene(&mut scene);

                Caption::paint(&mut app, &mut sb);

                text_cursor.paint(&mut app, &mut sb);
                text_cursor2.paint(&mut app, &mut sb);

                app.canvas.present(&scene, app.background);
            }

            Event::Key {
                key,
                state,
                modifiers,
            } => {
                text_cursor.on_key(key, state, modifiers);
                text_cursor2.on_key(key, state, modifiers);
            }

            Event::MouseButton {
                button,
                state,
                modifiers,
            } => {
                text_cursor.on_mouse(&mut app, button, state, modifiers);
                text_cursor2.on_mouse(&mut app, button, state, modifiers);
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
