use nari_gpu::vk::CommandBufferSubmitInfo;
use nari_ochre::euler::*;
use zeno::{
    apply, Cap, Command, Fill, Join, Mask, PathBuilder, PathData, Point, Stroke, Style, Transform,
};

#[derive(Copy, Clone)]
enum CurveOffset {
    None,
    Fixed { offset: f64 },
    Evolute,
}

fn euler_to(path: &mut Vec<Command>, euler: Euler, t0: f64, t1: f64, offset: CurveOffset) {
    let dt = 1.0 / euler.scale;

    let mut t = t0;
    while t < t1 {
        t = (t + dt).min(t1);
        let theta = euler_angle(euler, t);
        let mut pt = euler_eval(euler, 0.0, t);
        match offset {
            CurveOffset::None => (),
            CurveOffset::Fixed { offset } => {
                pt = pt + euler_normal(euler, t, offset);
            }
            CurveOffset::Evolute => {
                pt = pt + euler_normal(euler, t, euler_curvature_radius(euler, t));
            }
        }
        path.line_to(pt);
    }
}

fn main() {
    // let style = Style::Fill(Fill::NonZero);
    let style = Style::Stroke(Stroke {
        width: 1.5,
        join: Join::Round,
        miter_limit: 0.0,
        start_cap: Cap::Round,
        end_cap: Cap::Round,
        dashes: &[],
        offset: 0.0,
        scale: true,
    });

    let dpi: f64 = 1.0;
    let euler = Euler {
        p: Point::new(0.0, 0.0),
        scale: dpi * 300.0,
        k: [0.0, 1.0, -6.0],
    };
    let mut euler_inv = euler_inverse(euler);
    euler_inv.p = euler_eval(euler, 0.0, 1.0);
    dbg!(euler_eval(euler_inv, 0.0, 1.0));

    let width = 200.0;
    let offset = width * dpi;

    let mut path = Vec::new();

    let dt = 1.0 / euler.scale;

    path.move_to(euler.p);

    // positive side
    let c0 = ((1.0 - euler.k[1] * offset / euler.scale) / (euler.k[2] * offset / euler.scale))
        .min(1.0)
        .max(0.0);
    dbg!(c0);
    if c0 > 0.0 {
        let winding_p0 = euler.k[1] * offset / euler.scale < 1.0;
        dbg!(winding_p0);
        if winding_p0 {
            // offset forward
            path.line_to(euler.p + euler_normal(euler, 0.0, offset));
            euler_to(&mut path, euler, 0.0, c0, CurveOffset::Fixed { offset });
        } else {
            // evolute forward
            path.line_to(euler.p + euler_normal(euler, 0.0, euler_curvature_radius(euler, 0.0)));
            euler_to(&mut path, euler, 0.0, c0, CurveOffset::Evolute);
            // offset backward
            let p = euler_eval(euler, 0.0, c0) + euler_normal(euler, c0, offset);
            path.line_to(p);
            euler_to(
                &mut path,
                euler_inv,
                1.0 - c0,
                1.0,
                CurveOffset::Fixed { offset: -offset },
            );
            // evolute forward
            euler_to(&mut path, euler, 0.0, c0, CurveOffset::Evolute);
        }
    }
    if c0 < 1.0 {
        let winding_p1 = (euler.k[1] + euler.k[2]) * offset / euler.scale < 1.0;
        dbg!(winding_p1);
        let mut t: f64 = c0;
        if winding_p1 {
            // offset forward
            euler_to(&mut path, euler, c0, 1.0, CurveOffset::Fixed { offset });
        } else {
            // evolute forward
            path.line_to(
                euler_eval(euler, 0.0, t)
                    + euler_normal(euler, t, euler_curvature_radius(euler, t)),
            );
            euler_to(&mut path, euler, c0, 1.0, CurveOffset::Evolute);
            // offset backward
            path.line_to(euler_eval(euler, 0.0, 1.0) + euler_normal(euler, 1.0, offset));
            euler_to(
                &mut path,
                euler_inv,
                0.0,
                1.0 - c0,
                CurveOffset::Fixed { offset: -offset },
            );
            // evolute forward
            euler_to(&mut path, euler, c0, 1.0, CurveOffset::Evolute);
        }
    }

    // negative side
    let offset = -offset;
    let c0 = ((1.0 - euler.k[1] * offset / euler.scale) / (euler.k[2] * offset / euler.scale))
        .min(1.0)
        .max(0.0);
    dbg!(c0);
    if c0 < 1.0 {
        let winding_p1 = (euler.k[1] + euler.k[2]) * offset / euler.scale < 1.0;
        dbg!(winding_p1);
        if winding_p1 {
            // offset backward
            path.line_to(euler_eval(euler, 0.0, 1.0) + euler_normal(euler, 1.0, offset));
            euler_to(
                &mut path,
                euler_inv,
                0.0,
                1.0 - c0,
                CurveOffset::Fixed { offset: -offset },
            );
        } else {
            // evolute backward
            path.line_to(
                euler_eval(euler, 0.0, 1.0)
                    + euler_normal(euler, 1.0, euler_curvature_radius(euler, 1.0)),
            );
            euler_to(&mut path, euler_inv, 0.0, 1.0 - c0, CurveOffset::Evolute);
            // offset forward
            path.line_to(euler_eval(euler, 0.0, c0) + euler_normal(euler, c0, offset));
            euler_to(&mut path, euler, c0, 1.0, CurveOffset::Fixed { offset });
            // evolute backward
            euler_to(&mut path, euler_inv, 0.0, 1.0 - c0, CurveOffset::Evolute);
        }
    }
    if c0 > 0.0 {
        let winding_p0 = euler.k[1] * offset / euler.scale < 1.0;
        dbg!(winding_p0);
        if winding_p0 {
            // offset backward
            euler_to(
                &mut path,
                euler_inv,
                1.0 - c0,
                1.0,
                CurveOffset::Fixed { offset: -offset },
            );
        } else {
            // evolute backward
            euler_to(&mut path, euler_inv, 1.0 - c0, 1.0, CurveOffset::Evolute);
            // offset forward
            path.line_to(euler_eval(euler, 0.0, 0.0) + euler_normal(euler, 0.0, offset));
            euler_to(&mut path, euler, 0.0, c0, CurveOffset::Fixed { offset });
            // evolute backward
            euler_to(&mut path, euler_inv, 1.0 - c0, 1.0, CurveOffset::Evolute);
        }
    }

    path.line_to(euler.p);

    path.move_to(euler.p);
    euler_to(&mut path, euler, 0.0, 1.0, CurveOffset::None);

    // dbg!(&path);

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
