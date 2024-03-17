use nari_platform::{EventLoop, Extent, SurfaceArea};
use nari_vello::{
    icon::Icon, kurbo::Affine, kurbo::Point, kurbo::Rect, peniko::Brush, peniko::Color,
    peniko::Fill, Canvas, Scene,
};

pub struct Theme {
    icon_chrome_close: Icon,
    icon_chrome_minimize: Icon,
    icon_chrome_maximize: Icon,
    icon_chrome_restore: Icon,
}

impl Theme {
    const BUTTON_WIDTH: f64 = 46.0;
    const BUTTON_HEIGHT: f64 = 28.0;
    const CAPTION_HEIGHT: f64 = Self::BUTTON_HEIGHT;

    pub fn new() -> anyhow::Result<Self> {
        let icon_chrome_close = Icon::build(&std::fs::read("assets/codicon/chrome-close.svg")?)?;
        let icon_chrome_minimize =
            Icon::build(&std::fs::read("assets/codicon/chrome-minimize.svg")?)?;
        let icon_chrome_maximize =
            Icon::build(&std::fs::read("assets/codicon/chrome-maximize.svg")?)?;
        let icon_chrome_restore =
            Icon::build(&std::fs::read("assets/codicon/chrome-restore.svg")?)?;

        Ok(Self {
            icon_chrome_close,
            icon_chrome_minimize,
            icon_chrome_maximize,
            icon_chrome_restore,
        })
    }

    fn button_minimize(extent: Extent, dpi: f64) -> Rect {
        Rect {
            x0: extent.width - 3.0 * dpi * Self::BUTTON_WIDTH,
            x1: extent.width - 2.0 * dpi * Self::BUTTON_WIDTH,
            y0: 0.0,
            y1: dpi * Self::BUTTON_HEIGHT,
        }
    }

    fn button_maximize(extent: Extent, dpi: f64) -> Rect {
        Rect {
            x0: extent.width - 2.0 * dpi * Self::BUTTON_WIDTH,
            x1: extent.width - dpi * Self::BUTTON_WIDTH,
            y0: 0.0,
            y1: dpi * Self::BUTTON_HEIGHT,
        }
    }

    fn button_close(extent: Extent, dpi: f64) -> Rect {
        Rect {
            x0: extent.width - dpi * Self::BUTTON_WIDTH,
            x1: extent.width,
            y0: 0.0,
            y1: dpi * Self::BUTTON_HEIGHT,
        }
    }

    pub fn hit_test(&self, event_loop: &EventLoop, x: i32, y: i32) -> Option<SurfaceArea> {
        let dpi = event_loop.surface.dpi();

        let margin = (5.0 * dpi) as i32;
        let caption_height = (Self::CAPTION_HEIGHT * dpi) as i32;

        let extent = event_loop.surface.extent();
        let Extent { width, height } = extent;

        let w = width as i32;
        let h = height as i32;

        if !event_loop.surface.is_maximized() {
            // resize border
            match (x, y) {
                _ if x <= margin && y <= margin => return Some(SurfaceArea::TopLeft),
                _ if x >= w - margin && y <= margin => return Some(SurfaceArea::TopRight),
                _ if x >= w - margin && y >= h - margin => return Some(SurfaceArea::BottomRight),
                _ if x <= margin && y >= h - margin => return Some(SurfaceArea::BottomLeft),
                _ if x <= margin => return Some(SurfaceArea::Left),
                _ if y <= margin => return Some(SurfaceArea::Top),
                _ if x >= w - margin => return Some(SurfaceArea::Right),
                _ if y >= h - margin => return Some(SurfaceArea::Bottom),
                _ => (),
            };
        }

        let p = Point::new(x as _, y as _);

        let chrome_minimize = Self::button_minimize(extent, dpi);
        if chrome_minimize.contains(p) {
            return Some(SurfaceArea::Minimize);
        }

        let chrome_maximize = Self::button_maximize(extent, dpi);
        if chrome_maximize.contains(p) {
            return Some(SurfaceArea::Maximize);
        }

        let chrome_close = Self::button_close(extent, dpi);
        if chrome_close.contains(p) {
            return Some(SurfaceArea::Close);
        }

        if y <= caption_height {
            return Some(SurfaceArea::Caption);
        }

        None
    }

    pub fn paint(&self, event_loop: &EventLoop, canvas: &mut Canvas, mut sb: &mut Scene) {
        let extent = event_loop.surface.extent();
        let dpi = event_loop.surface.dpi();
        let affine_dpi = Affine::scale(dpi);

        let chrome_minimize = Self::button_minimize(extent, dpi);
        let chrome_maximize = Self::button_maximize(extent, dpi);
        let chrome_close = Self::button_close(extent, dpi);

        // hover background
        let mouse_pos = event_loop
            .mouse_position
            .map(|(x, y)| Point::new(x as f64, y as f64));
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

        let color_text = Color::rgb(0.0, 0.0, 0.0);

        let affine_minimize = Affine::translate(
            (chrome_minimize.center() - canvas.scale_pt(self.icon_chrome_minimize.bbox.center()))
                .floor(),
        );
        self.icon_chrome_minimize.paint(
            &mut sb,
            affine_minimize * affine_dpi,
            &Brush::Solid(color_text),
        );

        let icon_maximize = if event_loop.surface.is_maximized() {
            &self.icon_chrome_restore
        } else {
            &self.icon_chrome_maximize
        };
        let affine_maximize = Affine::translate(
            (chrome_maximize.center() - canvas.scale_pt(icon_maximize.bbox.center())).floor(),
        );
        icon_maximize.paint(
            &mut sb,
            affine_maximize * affine_dpi,
            &Brush::Solid(color_text),
        );

        let close_color = if let Some(p) = mouse_pos {
            if chrome_close.contains(p) {
                Color::rgb(1.0, 1.0, 1.0)
            } else {
                color_text
            }
        } else {
            color_text
        };
        let affine_close = Affine::translate(
            (chrome_close.center() - canvas.scale_pt(self.icon_chrome_close.bbox.center())).floor(),
        );
        self.icon_chrome_close.paint(
            &mut sb,
            affine_close * affine_dpi,
            &Brush::Solid(close_color),
        );
    }
}
