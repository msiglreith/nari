use nari_ochre::euler::*;
use zeno::{
    apply, Cap, Command, Fill, Join, Mask, PathBuilder, PathData, Point, Stroke, Style, Transform,
};

fn main() {
    let style = Style::Fill(Fill::NonZero);
    // let style = Style::Stroke(Stroke {
    //     width: 2.0,
    //     join: Join::Round,
    //     miter_limit: 0.0,
    //     start_cap: Cap::Round,
    //     end_cap: Cap::Round,
    //     dashes: &[],
    //     offset: 0.0,
    //     scale: true,
    // });

    let dpi: f64 = 1.0;
    let euler = Euler {
        p: Point::new(0.0, 0.0),
        scale: dpi * 300.15316943682936,
        k: [0.0, -10.0, 20.0],
    };
    let width = 100.0;
    let offset = width * dpi;

    let mut path = Vec::new();

    let dt = 1.0 / euler.scale;

    path.move_to(euler.p);

    // positive side
    let c0 =
        ((1.0 - euler.k[1] * offset / euler.scale) / (euler.k[2] * offset / euler.scale)).min(1.0);
    dbg!(c0);
    if c0 > 0.0 {
        let winding_p0 = euler.k[1] * offset / euler.scale < 0.0;
        dbg!(winding_p0);
        if winding_p0 {
            let mut t: f64 = 0.0;
            path.line_to(euler.p + euler_normal(euler, t, offset));
            while t < c0 {
                t = (t + dt).min(c0);
                let theta = euler_angle(euler, t);
                path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            }
        } else {
            let mut t: f64 = 0.0;
            path.line_to(euler.p + euler_normal(euler, t, euler_curvature_radius(euler, t)));
            while t < c0 {
                t = (t + dt).min(c0);
                let theta = euler_angle(euler, t);
                path.line_to(
                    euler_eval(euler, 0.0, t)
                        + euler_normal(euler, t, euler_curvature_radius(euler, t)),
                );
            }
        }
    }
    if c0 < 1.0 {
        let winding_p1 = (euler.k[1] + euler.k[2]) * offset / euler.scale < 0.0;
        dbg!(winding_p1);
        if winding_p1 {
            let mut t: f64 = c0;
            path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            while t < 1.0 {
                t = (t + dt).min(1.0);
                let theta = euler_angle(euler, t);
                path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            }
        } else {
            let mut t: f64 = c0;
            path.line_to(
                euler_eval(euler, 0.0, t)
                    + euler_normal(euler, t, euler_curvature_radius(euler, t)),
            );
            while t < 1.0 {
                t = (t + dt).min(1.0);
                let theta = euler_angle(euler, t);
                path.line_to(
                    euler_eval(euler, 0.0, t)
                        + euler_normal(euler, t, euler_curvature_radius(euler, t)),
                );
            }
        }
    }

    let mut t = 1.0;
    path.line_to(euler_eval(euler, 0.0, t));
    while t > 0.0 {
        t = (t - dt).max(0.0);
        path.line_to(euler_eval(euler, 0.0, t));
    }

    // negative side
    let offset = -offset;
    let c0 =
        ((1.0 - euler.k[1] * offset / euler.scale) / (euler.k[2] * offset / euler.scale)).min(1.0);
    dbg!(c0);
    if c0 > 0.0 {
        let winding_p0 = euler.k[1] * offset / euler.scale < 0.0;
        dbg!(winding_p0);
        if winding_p0 {
            let mut t: f64 = 0.0;
            path.line_to(euler.p + euler_normal(euler, t, offset));
            while t < c0 {
                t = (t + dt).min(c0);
                let theta = euler_angle(euler, t);
                path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            }
        } else {
            let mut t: f64 = 0.0;
            path.line_to(euler.p + euler_normal(euler, t, euler_curvature_radius(euler, t)));
            while t < c0 {
                t = (t + dt).min(c0);
                let theta = euler_angle(euler, t);
                path.line_to(
                    euler_eval(euler, 0.0, t)
                        + euler_normal(euler, t, euler_curvature_radius(euler, t)),
                );
            }
        }
    }
    if c0 < 1.0 {
        let winding_p1 = (euler.k[1] + euler.k[2]) * offset / euler.scale < 0.0;
        dbg!(winding_p1);
        if winding_p1 {
            let mut t: f64 = c0;
            path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            while t < 1.0 {
                t = (t + dt).min(1.0);
                let theta = euler_angle(euler, t);
                path.line_to(euler_eval(euler, 0.0, t) + euler_normal(euler, t, offset));
            }
        } else {
            let mut t: f64 = c0;
            path.line_to(
                euler_eval(euler, 0.0, t)
                    + euler_normal(euler, t, euler_curvature_radius(euler, t)),
            );
            while t < 1.0 {
                t = (t + dt).min(1.0);
                let theta = euler_angle(euler, t);
                path.line_to(
                    euler_eval(euler, 0.0, t)
                        + euler_normal(euler, t, euler_curvature_radius(euler, t)),
                );
            }
        }
    }

    let mut t = 1.0;
    path.line_to(euler_eval(euler, 0.0, t));
    while t > 0.0 {
        t = (t - dt).max(0.0);
        path.line_to(euler_eval(euler, 0.0, t));
    }

    let mut stroke = Vec::new();
    apply(&path, style, None, &mut stroke);
    let mask = Mask::new(&stroke);
    let (mask, place) = mask.render();
    dbg!(place);
    image::save_buffer(
        &format!("path.zeno.png"),
        &mask,
        place.width,
        place.height,
        image::ColorType::L8,
    )
    .unwrap();
}
