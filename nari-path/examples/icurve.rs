use nari_platform::{ControlFlow, Event, KeyState, MouseButtons, Platform};
use nari_vello::{
    kurbo::{Affine, Circle, Point, QuadBez, Rect, Stroke, Vec2},
    peniko::{Brush, Color, Fill},
    Canvas, Scene,
};

#[derive(Copy, Clone, Debug)]
struct Curve2 {
    p0: Point,
    p1: Point,
    p2: Point,
}

impl Curve2 {
    fn eval(&self, t: f64) -> Point {
        let tt = 1.0 - t;
        (self.p0.to_vec2() * (tt * tt)
            + (self.p1.to_vec2() * (tt * 2.0) + self.p2.to_vec2() * t) * t)
            .to_point()
    }

    fn monotonize(&self) -> Vec<Self> {
        let a = self.p0.to_vec2() - 2.0 * self.p1.to_vec2() + self.p2.to_vec2();
        let b = self.p1 - self.p0;

        fn intersection(a: f64, b: f64) -> Option<f64> {
            if a.signum() != b.signum() && b != 0.0 && b.abs() < a.abs() {
                return Some(-b / a);
            }
            None
        }

        let mut roots = Vec::new();
        if let Some(root) = intersection(a.x, b.x) {
            roots.push(root);
        }
        if let Some(root) = intersection(a.y, b.y) {
            roots.push(root);
        }
        roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
        roots.push(1.0);

        let mut curves = Vec::new();
        let mut prev = self.p0;
        let mut t_prev = 0.0;
        for i in 0..roots.len() {
            let t = roots[i];
            let p0 = prev;
            let p2 = self.eval(t);
            let p1 = p0 + (self.p1 - self.p0).lerp(self.p2 - self.p1, t_prev) * (t - t_prev);
            curves.push(Curve2 { p0, p1, p2 });
            prev = p2;
            t_prev = t;
        }

        curves
    }
}

enum TreeElement {
    Segment(usize),
    Winding(isize),
}

struct TreeCell {
    rect: Rect,
    elements: Vec<TreeElement>,
}

struct Tree {
    rect: Rect,
    segments: Vec<Curve2>,
    cells: Vec<TreeCell>,
}

fn ccw(p1: Vec2, p0: Vec2, p2: Vec2) -> f64 {
    (p1.x - p0.x) * (p2.y - p0.y) - (p1.y - p0.y) * (p2.x - p0.x)
}

impl Tree {
    fn new(segments: Vec<Curve2>) -> Self {
        let mut rect = Rect {
            x0: segments[0].p0.x,
            x1: segments[0].p0.x,
            y0: segments[0].p0.y,
            y1: segments[0].p0.y,
        };
        for segment in &segments {
            rect = rect.union_pt(segment.p0).union_pt(segment.p2);
        }
        let elements = (0..segments.len())
            .map(|i| TreeElement::Segment(i))
            .collect();

        let mut tree = Tree {
            rect,
            segments,
            cells: vec![TreeCell { rect, elements }],
        };
        tree
    }

    fn split(&self) -> Self {
        let mut cells = Vec::new();
        for cell in &self.cells {
            let center = cell.rect.center();
            let tl = Rect {
                x0: cell.rect.x0,
                x1: center.x,
                y0: cell.rect.y0,
                y1: center.y,
            };
            let tr = Rect {
                x0: center.x,
                x1: cell.rect.x1,
                y0: cell.rect.y0,
                y1: center.y,
            };
            let bl = Rect {
                x0: cell.rect.x0,
                x1: center.x,
                y0: center.y,
                y1: cell.rect.y1,
            };
            let br = Rect {
                x0: center.x,
                x1: cell.rect.x1,
                y0: center.y,
                y1: cell.rect.y1,
            };

            for element in &cell.elements {
                match element {
                    TreeElement::Segment(i) => {
                        let segment = &self.segments[*i];
                    }
                    TreeElement::Winding(winding) => {}
                }
            }
        }

        Self {
            rect: self.rect,
            segments: self.segments.clone(),
            cells,
        }
    }

    fn eval(&self, pt: Point, sb: &mut Scene) {
        let mut winding = 0;
        for cell in &self.cells {
            if cell.rect.contains(pt) {
                for element in &cell.elements {
                    match element {
                        TreeElement::Segment(i) => {
                            let segment = &self.segments[*i];
                            let Curve2 { p0, p1, p2 } = segment;

                            let is_up = p2.y > p0.y;
                            let is_right = p2.x > p0.x;
                            let is_c_left = ccw(p1.to_vec2(), p0.to_vec2(), p2.to_vec2()) < 0.0;
                            let is_negative = is_c_left == is_up;
                            let is_out_left = is_up != is_c_left;
                            let factor = if is_negative { -1.0 } else { 1.0 };

                            let winding_local = if is_up { 1 } else { -1 };

                            // aabb hit test
                            if pt.y < p0.y.min(p2.y) {
                                continue;
                            }
                            if pt.y >= p0.y.max(p2.y) {
                                continue;
                            }
                            if pt.x >= p0.x.max(p2.x) {
                                continue;
                            }

                            if pt.x < p0.x.min(p2.x) {
                                winding += winding_local;
                            } else {
                                // inside

                                let sample_left =
                                    ccw(pt.to_vec2(), p0.to_vec2(), p2.to_vec2()) < 0.0;
                                if sample_left == is_c_left {
                                    // implicit transformation for quadratic bezier curve
                                    let det_inv = 1.0
                                        / (-p0.y * p1.x + p0.x * p1.y + p0.y * p2.x
                                            - p1.y * p2.x
                                            - p0.x * p2.y
                                            + p1.x * p2.y);
                                    let m00 =
                                        det_inv * (p0.y - p1.y) + det_inv * (p2.y - p0.y) / 2.0;
                                    let m01 =
                                        det_inv * (p1.x - p0.x) + det_inv * (p0.x - p2.x) / 2.0;
                                    let m10 = det_inv * (p0.y - p1.y);
                                    let m11: f64 = det_inv * (p1.x - p0.x);

                                    let px = pt.x as f64 - p0.x;
                                    let py = pt.y as f64 - p0.y;

                                    let u = m00 * px + m01 * py;
                                    let v = m10 * px + m11 * py;

                                    // evaluate implicit function
                                    let f = u * u - v;

                                    if f * factor < 0.0 {
                                        winding += winding_local;
                                    }
                                } else if is_out_left {
                                    winding += winding_local;
                                }
                            }
                        }
                        TreeElement::Winding(w) => {
                            winding += w;
                        }
                    }
                }
            }
        }

        if winding != 0 {
            sb.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &Brush::Solid(Color::rgb(0.2, 1.0, 0.2)),
                None,
                &Circle::new(pt, 4.0),
            );
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let platform = Platform::new();
    let theme = nari_decor_basic::Theme::new()?;

    let mut canvas = Canvas::new(platform.surface).await;
    let mut scene = Scene::default();

    let mut q = Curve2 {
        p0: Point::new(900.0, 950.0),
        p1: Point::new(700.0, 900.0),
        p2: Point::new(500.0, 500.0),
    };

    let mut active_handle = None;

    platform.run(move |event_loop, event| {
        match event {
            Event::Resize(extent) => {
                canvas.resize(extent);
                event_loop.surface.redraw();
            }

            Event::Hittest { x, y, area } => {
                if let Some(area_decor) = theme.hit_test(&event_loop, x, y) {
                    *area = area_decor;
                }
            }

            Event::MouseButton { button, state, .. } => {
                if let Some((px, py)) = event_loop.mouse_position {
                    if button == MouseButtons::LEFT {
                        if state == KeyState::Down {
                            let p = Point::new(px as f64, py as f64);
                            let margin = canvas.scale(8.0);

                            if q.p0.distance(p) < margin {
                                active_handle = Some(0);
                            } else if q.p1.distance(p) < margin {
                                active_handle = Some(1);
                            } else if q.p2.distance(p) < margin {
                                active_handle = Some(2);
                            }
                        } else {
                            active_handle = None;
                        }
                    }
                }
            }

            Event::MouseMove => {
                if let Some((px, py)) = event_loop.mouse_position {
                    let p = Point::new(px as f64, py as f64);

                    match active_handle {
                        Some(0) => q.p0 = p,
                        Some(1) => q.p1 = p,
                        Some(2) => q.p2 = p,
                        _ => (),
                    }
                    event_loop.surface.redraw();
                }
            }

            Event::Paint => {
                let mut sb = &mut scene;
                sb.reset();

                let segments = q.monotonize();
                let colors = [
                    Color::rgb(1.0, 0.0, 0.0),
                    Color::rgb(0.0, 1.0, 0.0),
                    Color::rgb(0.0, 0.0, 1.0),
                ];

                let mut tree = Tree::new(segments);

                sb.stroke(
                    &Stroke::new(2.0),
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.0, 0.0, 0.0)),
                    None,
                    &tree.rect,
                );

                for i in 0..tree.segments.len() {
                    let seg = &tree.segments[i];
                    sb.stroke(
                        &Stroke::new(2.0),
                        Affine::IDENTITY,
                        &Brush::Solid(colors[i % colors.len()]),
                        None,
                        &QuadBez::new(seg.p0, seg.p1, seg.p2),
                    );
                }

                // sb.stroke(&Stroke::new(2.0), Affine::IDENTITY, &Brush::Solid(Color::rgb(0.0, 0.0, 0.0)), None, &QuadBez::new(q.p0, q.p1, q.p2));
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.0, 0.0, 0.0)),
                    None,
                    &Circle::new(q.p0, 4.0),
                );
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.0, 0.0, 0.0)),
                    None,
                    &Circle::new(q.p1, 4.0),
                );
                sb.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &Brush::Solid(Color::rgb(0.0, 0.0, 0.0)),
                    None,
                    &Circle::new(q.p2, 4.0),
                );

                if let Some((px, py)) = event_loop.mouse_position {
                    let p = Point::new(px as f64, py as f64);

                    tree.eval(p, sb);
                }

                theme.paint(&event_loop, &mut canvas, &mut sb);
                canvas.present(&scene, Color::rgb(1.0, 1.0, 1.0));
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
