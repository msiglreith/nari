use nari_platform::{EventLoop, MouseButtons, Platform};
use nari_vello::{icon::Icon, peniko::Color, typo::FontScaled, Canvas};
use parley::fontique::Attributes;

pub struct Style {
    pub font_regular: FontScaled,

    pub logo: Icon,

    pub icon_chrome_minimize: Icon,
    pub icon_chrome_maximize: Icon,
    pub icon_chrome_restore: Icon,
    pub icon_chrome_close: Icon,

    pub color_caption: Color,
    pub color_text: Color,
    pub color_background: Color,
    pub color_text_select: Color,
    pub color_cursor: Color,
}

pub struct App {
    pub canvas: Canvas,
    pub event_loop: EventLoop,
    pub style: Style,
}

impl App {
    pub async fn new(platform: &Platform) -> anyhow::Result<Self> {
        let mut canvas = Canvas::new(platform.surface).await;

        let icon_chrome_close = Icon::build(&std::fs::read("assets/codicon/chrome-close.svg")?)?;
        let icon_chrome_minimize =
            Icon::build(&std::fs::read("assets/codicon/chrome-minimize.svg")?)?;
        let icon_chrome_maximize =
            Icon::build(&std::fs::read("assets/codicon/chrome-maximize.svg")?)?;
        let icon_chrome_restore =
            Icon::build(&std::fs::read("assets/codicon/chrome-restore.svg")?)?;

        let logo = Icon::build(&std::fs::read("assets/logo.svg")?)?;

        let font_body = canvas.create_font(std::fs::read("assets/Lato/Lato-Regular.ttf")?);
        let font_body_regular = canvas.create_font_scaled(
            font_body,
            canvas.scale(16.0).round() as u32,
            Attributes::default(),
        );

        Ok(Self {
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
        })
    }
}
