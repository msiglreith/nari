// use nari_canvas as canvas;
// use nari_gpu as gpu;
// use std::collections::VecDeque;
// use std::path::Path;
// use unicode_segmentation::GraphemeCursor;
// use winit::{
//     dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
//     event::{
//         ElementState, Event, KeyboardInput, ModifiersState, MouseButton, VirtualKeyCode,
//         WindowEvent,
//     },
//     event_loop::{ControlFlow, EventLoop},
//     platform::windows::WindowBuilderExtWindows,
//     window::{WindowArea, WindowBuilder},
// };

// const CURSOR: canvas::Color = [0.0, 0.95, 1.0, 1.0];
// const TEXT_DEFAULT: canvas::Color = [1.0, 1.0, 1.0, 1.0];
// const TEXT_SELECT: canvas::Color = [0.15, 0.17, 0.22, 1.0];
// const BACKGROUND: canvas::Color = [0.12, 0.14, 0.17, 1.0];

// const CAPTION_HEIGHT: u32 = 29;
// const CLOSE_WIDTH: u32 = 45;

// // Pretty simple cursor, might not be ideal for more complex scripts
// // with grapheme clusters.
// struct TextCursor {
//     font: canvas::typo::FontScaled,
//     pen: canvas::typo::Pen,
//     pos: usize,
//     text: String,
//     focused: bool,
// }

// impl TextCursor {
//     fn on_char(&mut self, c: char) {
//         if c.is_control() {
//             return;
//         }

//         self.text.insert(self.pos, c);
//         self.pos += c.len_utf8();
//     }

//     fn on_key(&mut self, key: VirtualKeyCode) {
//         match key {
//             VirtualKeyCode::Back => {
//                 // grapheme based, could be more sophisticated
//                 let mut cursor = GraphemeCursor::new(self.pos, self.text.len(), true);
//                 if let Some(pos) = cursor.prev_boundary(&self.text, 0).unwrap() {
//                     let new_pos = pos;
//                     for _ in new_pos..=pos {
//                         self.text.remove(new_pos);
//                     }
//                     self.pos = new_pos;
//                 }
//             }
//             VirtualKeyCode::Left => {
//                 let mut cursor = GraphemeCursor::new(self.pos, self.text.len(), true);
//                 if let Some(pos) = cursor.prev_boundary(&self.text, 0).unwrap() {
//                     self.pos = pos;
//                 }
//             }
//             VirtualKeyCode::Right => {
//                 let mut cursor = GraphemeCursor::new(self.pos, self.text.len(), true);
//                 if let Some(pos) = cursor.next_boundary(&self.text, 0).unwrap() {
//                     self.pos = pos;
//                 }
//             }
//             _ => (),
//         }
//     }

//     fn update(&mut self, ui: &mut Ui) {
//         // input handling
//         for event in &ui.input.events {
//             match event {
//                 InputEvent::Mouse {
//                     button: MouseButton::Left,
//                     state: ElementState::Pressed,
//                     ..
//                 } => {
//                     let text_run = ui.canvas.layout_text(self.font, &self.text);
//                     if let Some(cursor) = ui.input.cursor_pos {
//                         if let Some(canvas::typo::Caret { cluster }) = text_run.hittest(
//                             self.pen,
//                             cursor.x.round() as i32,
//                             cursor.y.round() as i32,
//                         ) {
//                             self.focused = true;
//                             self.pos = if cluster == text_run.clusters.len() {
//                                 self.text.len()
//                             } else {
//                                 text_run.clusters[cluster].byte_pos
//                             };
//                         } else {
//                             self.focused = false;
//                         }
//                     }
//                 }
//                 InputEvent::Text { c } if self.focused => self.on_char(*c),
//                 InputEvent::Keyboard {
//                     key,
//                     state: ElementState::Pressed,
//                     ..
//                 } if self.focused => {
//                     self.on_key(*key);
//                 }
//                 _ => (),
//             }
//         }

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

// enum InputEvent {
//     Mouse {
//         button: MouseButton,
//         state: ElementState,
//         // modifiers: ModifiersState,
//     },
//     Keyboard {
//         key: VirtualKeyCode,
//         state: ElementState,
//         // modifiers: ModifiersState,
//     },
//     Text {
//         c: char,
//     },
// }

// struct FrameInput {
//     size: PhysicalSize<u32>,
//     cursor_pos: Option<PhysicalPosition<f64>>,
//     events: Vec<InputEvent>,
//     // caching currently active modifiers
//     modifiers: ModifiersState,
// }

// impl FrameInput {
//     fn area(&self) -> gpu::Rect2D {
//         gpu::Rect2D {
//             offset: gpu::Offset2D { x: 0, y: 0 },
//             extent: gpu::Extent2D {
//                 width: self.size.width,
//                 height: self.size.height,
//             },
//         }
//     }
// }

// fn main() -> anyhow::Result<()> {
//     use winit::platform::windows::WindowExtWindows;

//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new()
//         .with_title("nari :: studio")
//         .with_decorations(false)
//         .with_undecorated_shadow(true)
//         .with_inner_size(LogicalSize::new(1440.0f32, 800.0))
//         .build(&event_loop)?;

//     unsafe {

//         let font_regular = ui
//             .canvas
//             .create_font(std::fs::read("assets/Inter/Inter-Regular.ttf")?);
//         let font_regular = ui.canvas.create_font_scaled(font_regular, 15);

//         let mut text_cursor = TextCursor {
//             font: font_regular,
//             pen: canvas::typo::Pen {
//                 x: 10,
//                 y: 30,
//                 color: TEXT_DEFAULT,
//                 align_x: canvas::Align::Positive,
//             },
//             pos: 0,
//             text: "hello world! ".to_string(),
//             focused: false,
//         };

//         let mut text_cursor2 = TextCursor {
//             font: font_regular,
//             pen: canvas::typo::Pen {
//                 x: 10,
//                 y: 50,
//                 color: TEXT_DEFAULT,
//                 align_x: canvas::Align::Positive,
//             },
//             pos: 0,
//             text: "test row 2".to_string(),
//             focused: false,
//         };

//         event_loop.run(move |event, _, control_flow| {
//             *control_flow = ControlFlow::Wait;

//             match event {
//                 Event::WindowEvent { event, .. } => match event {
//                     WindowEvent::Resized(_) => {
//                         ui.resize(window.inner_size());
//                     }
//                     WindowEvent::ReceivedCharacter(c) => {
//                         ui.input.events.push(InputEvent::Text { c });
//                     }
//                     WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
//                     WindowEvent::KeyboardInput {
//                         input:
//                             KeyboardInput {
//                                 state,
//                                 virtual_keycode: Some(key),
//                                 ..
//                             },
//                         ..
//                     } => {
//                         ui.input.events.push(InputEvent::Keyboard {
//                             state,
//                             key,
//                             // modifiers: ui.input.modifiers,
//                         });
//                     }
//                     WindowEvent::CursorMoved { position, .. } => {
//                         ui.input.cursor_pos = Some(position);
//                     }
//                     WindowEvent::CursorLeft { .. } => {
//                         ui.input.cursor_pos = None;
//                     }
//                     WindowEvent::ModifiersChanged(state) => {
//                         ui.input.modifiers = state;
//                     }
//                     WindowEvent::MouseInput { button, state, .. } => {
//                         ui.input.events.push(InputEvent::Mouse {
//                             state,
//                             button,
//                             // modifiers: ui.input.modifiers,
//                         });
//                     }
//                     WindowEvent::HitTest { x, y, new_area } => {
//                         let size = window.inner_size();
//                         let h = size.height;
//                         let w = size.width;

//                         const MARGIN: u32 = 5;

//                         let chrome_minimize = canvas::Rect {
//                             x0: size.width.saturating_sub(3 * CLOSE_WIDTH) as _,
//                             x1: size.width.saturating_sub(2 * CLOSE_WIDTH) as _,
//                             y0: 0,
//                             y1: CAPTION_HEIGHT as _,
//                         };
//                         let chrome_maximize = canvas::Rect {
//                             x0: size.width.saturating_sub(2 * CLOSE_WIDTH) as _,
//                             x1: size.width.saturating_sub(CLOSE_WIDTH) as _,
//                             y0: 0,
//                             y1: CAPTION_HEIGHT as _,
//                         };
//                         let chrome_close = canvas::Rect {
//                             x0: size.width.saturating_sub(CLOSE_WIDTH) as _,
//                             x1: size.width as _,
//                             y0: 0,
//                             y1: CAPTION_HEIGHT as _,
//                         };

//                         *new_area = match (x, y) {
//                             _ if x <= MARGIN && y <= MARGIN => WindowArea::TOPLEFT,
//                             _ if x >= w - MARGIN && y <= MARGIN => WindowArea::TOPRIGHT,
//                             _ if x >= w - MARGIN && y >= h - MARGIN => WindowArea::BOTTOMRIGHT,
//                             _ if x <= MARGIN && y >= h - MARGIN => WindowArea::BOTTOMLEFT,
//                             _ if x <= MARGIN => WindowArea::LEFT,
//                             _ if y <= MARGIN => WindowArea::TOP,
//                             _ if x >= w - MARGIN => WindowArea::RIGHT,
//                             _ if y >= h - MARGIN => WindowArea::BOTTOM,
//                             _ if chrome_minimize.hittest(x as _, y as _) => WindowArea::MINBUTTON,
//                             _ if chrome_maximize.hittest(x as _, y as _) => WindowArea::MAXBUTTON,
//                             _ if chrome_close.hittest(x as _, y as _) => WindowArea::CLOSE,
//                             (_, 0..=CAPTION_HEIGHT) => WindowArea::CAPTION,
//                             _ => WindowArea::CLIENT,
//                         }
//                     }
//                     _ => (),
//                 },
//                 Event::MainEventsCleared => {
//                     window.request_redraw();
//                 }
//                 Event::RedrawRequested(_) => {
//                     let frame = ui.begin_frame();

//                     let size = window.inner_size();

//                     text_cursor.update(&mut ui);
//                     text_cursor2.update(&mut ui);

//                     if let Some(cursor) = ui.input.cursor_pos {
//                         if chrome_minimize.hittest(cursor.x as _, cursor.y as _) {
//                             ui.canvas.rect(chrome_minimize, [0.27, 0.3, 0.34, 1.0]);
//                         } else if chrome_maximize.hittest(cursor.x as _, cursor.y as _) {
//                             ui.canvas.rect(chrome_maximize, [0.27, 0.3, 0.34, 1.0]);
//                         } else if chrome_close.hittest(cursor.x as _, cursor.y as _) {
//                             ui.canvas.rect(chrome_close, [0.9, 0.07, 0.14, 1.0]);
//                         }
//                     }

//                     ui.end_frame(frame);
//                 }
//                 Event::RedrawEventsCleared => {}
//                 _ => (),
//             }
//         })
//     }
// }

use nari_platform::{
    ControlFlow, Event, EventLoop, Extent, MouseButtons, Platform, Surface, SurfaceArea,
};
use nari_vello::{
    kurbo::{Affine, Point, Rect},
    peniko::{Brush, Color, Fill},
    typo::{Font, FontScaled},
    Canvas, Codicon, Scene, SceneBuilder, SceneFragment,
};

struct App {
    canvas: Canvas,
    event_loop: EventLoop,

    codicon: Font,
    codicon_regular: FontScaled,

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

    fn paint(app: &mut App, mut sb: &mut SceneBuilder) {
        let extent = app.event_loop.surface.extent();

        let chrome_minimize = Self::button_minimize(extent);
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

        let chrome_maximize = Self::button_maximize(extent);
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

        let chrome_close = Self::button_close(extent);
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
        background,
        foreground,
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
                }
            }

            Event::Paint => {
                let size = app.event_loop.surface.extent();

                let mut sb = SceneBuilder::for_scene(&mut scene);
                Caption::paint(&mut app, &mut sb);

                app.canvas.present(&scene, app.background);
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
