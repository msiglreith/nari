use zeno::{Point, Vector};

#[derive(Copy, Clone, Debug)]
pub struct Euler {
    pub p: Point,
    pub scale: f64,
    pub k: [f64; 3],
}

// Adopted from kurbo

// Copyright (c) 2018 Raph Levien

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

fn spiro(k0: f64, k1: f64) -> (f64, f64) {
    let t1_1 = k0;
    let t1_2 = 0.5 * k1;
    let t2_2 = t1_1 * t1_1;
    let t2_3 = 2. * (t1_1 * t1_2);
    let t2_4 = t1_2 * t1_2;
    let t3_4 = t2_2 * t1_2 + t2_3 * t1_1;
    let t3_6 = t2_4 * t1_2;
    let t4_4 = t2_2 * t2_2;
    let t4_5 = 2. * (t2_2 * t2_3);
    let t4_6 = 2. * (t2_2 * t2_4) + t2_3 * t2_3;
    let t4_7 = 2. * (t2_3 * t2_4);
    let t4_8 = t2_4 * t2_4;
    let t5_6 = t4_4 * t1_2 + t4_5 * t1_1;
    let t5_8 = t4_6 * t1_2 + t4_7 * t1_1;
    let t5_10 = t4_8 * t1_2;
    let t6_6 = t4_4 * t2_2;
    let t6_7 = t4_4 * t2_3 + t4_5 * t2_2;
    let t6_8 = t4_4 * t2_4 + t4_5 * t2_3 + t4_6 * t2_2;
    let t6_9 = t4_5 * t2_4 + t4_6 * t2_3 + t4_7 * t2_2;
    let t6_10 = t4_6 * t2_4 + t4_7 * t2_3 + t4_8 * t2_2;
    let t7_8 = t6_6 * t1_2 + t6_7 * t1_1;
    let t7_10 = t6_8 * t1_2 + t6_9 * t1_1;
    let t8_8 = t6_6 * t2_2;
    let t8_9 = t6_6 * t2_3 + t6_7 * t2_2;
    let t8_10 = t6_6 * t2_4 + t6_7 * t2_3 + t6_8 * t2_2;
    let t9_10 = t8_8 * t1_2 + t8_9 * t1_1;
    let t10_10 = t8_8 * t2_2;
    let mut u = 1.;
    u -= (1. / 24.) * t2_2 + (1. / 160.) * t2_4;
    u += (1. / 1920.) * t4_4 + (1. / 10752.) * t4_6 + (1. / 55296.) * t4_8;
    u -= (1. / 322560.) * t6_6 + (1. / 1658880.) * t6_8 + (1. / 8110080.) * t6_10;
    u += (1. / 92897280.) * t8_8 + (1. / 454164480.) * t8_10;
    u -= 2.4464949595157930e-11 * t10_10;
    let mut v = (1. / 12.) * t1_2;
    v -= (1. / 480.) * t3_4 + (1. / 2688.) * t3_6;
    v += (1. / 53760.) * t5_6 + (1. / 276480.) * t5_8 + (1. / 1351680.) * t5_10;
    v -= (1. / 11612160.) * t7_8 + (1. / 56770560.) * t7_10;
    v += 2.4464949595157932e-10 * t9_10;
    (u, v)
}

fn spiro_n(k0: f64, k1: f64) -> (f64, f64) {
    const ACCURACY: f64 = 1.0e-20;

    let c1 = k1.abs();
    let c0 = k0.abs() + 0.5 * c1;
    let est_err_raw = 0.006 * c0 * c0 + 0.029 * c1;
    if est_err_raw.powi(6) < ACCURACY {
        spiro(k0, k1)
    } else {
        let n = (est_err_raw / ACCURACY.powf(1.0 / 6.0)).ceil() as usize;
        let s1s0 = 1.0 / n as f64;

        let mut px = 0.0;
        let mut py = 0.0;

        // s0 = -0.5, s1 = -0.5 + s1s0, s = (s0 + s1) * 0.5
        let mut s = -0.5 + 0.5 * s1s0;

        for _ in 0..n {
            let (u, v) = spiro(s1s0 * (k0 + k1 * s), s1s0 * s1s0 * k1);

            let theta = k0 * s + 0.5 * k1 * s * s;
            let (ts, tc) = theta.sin_cos();

            px += tc * u - ts * v;
            py += tc * v + ts * u;

            s += s1s0
        }

        (px * s1s0, py * s1s0)
    }
}

pub fn euler_angle(euler: Euler, t: f64) -> f64 {
    0.5 * euler.k[2] * t * t + euler.k[1] * t + euler.k[0]
}

pub fn euler_curvature_radius(euler: Euler, t: f64) -> f64 {
    euler.scale / (euler.k[2] * t + euler.k[1])
}

pub fn euler_normal(euler: Euler, t: f64, offset: f64) -> Vector {
    let theta = euler_angle(euler, t);
    let (ts, tc) = theta.sin_cos();
    Vector::new((-offset * ts) as f32, (offset * tc) as f32)
}

pub fn euler_eval(euler: Euler, t0: f64, t1: f64) -> Point {
    let s1s0 = t1 - t0;
    let s = (t0 + t1) * 0.5;
    let (u, v) = spiro_n(
        s1s0 * (euler.k[2] * s + euler.k[1]),
        s1s0 * s1s0 * euler.k[2],
    );
    let theta = euler_angle(euler, s);
    let (ts, tc) = theta.sin_cos();
    let x = s1s0 * (tc * u - ts * v);
    let y = s1s0 * (tc * v + ts * u);
    euler.p + Point::new((euler.scale * x) as f32, (euler.scale * y) as f32)
}

fn euler_fit(p0: Point, len: f64, th: f64, th0: f64, th1: f64) -> Euler {
    let mut k2_old = 0.0;
    let mut e_old = th1 - th0;
    let k1 = th0 + th1;
    let mut k2 = 6.0 * (1.0 - ((0.5 / std::f64::consts::PI) * k1).powi(3)) * e_old;
    let mut x = 0.0;
    let mut y = 0.0;
    for _ in 0..10 {
        (x, y) = spiro(k1, k2);
        let theta = y.atan2(x);
        let e = (th1 - th0) + 2.0 * theta - 0.25 * k2;
        if e.abs() < 1e-9 {
            break;
        }

        let new_k2 = k2 + (k2_old - k2) * e / (e - e_old);
        k2_old = k2;
        e_old = e;
        k2 = new_k2;
    }

    let chord = (x * x + y * y).sqrt();
    let scale = len / chord;

    let c0 = th - y.atan2(x);

    // substitue from -0.5:0.5 to 0:1
    let u0 = c0 - 0.5 * k1 + 0.125 * k2;
    let u1 = k1 - k2 * 0.5;
    let u2 = k2;

    Euler {
        p: p0,
        scale,
        k: [u0, u1, u2],
    }
}

pub fn euler_fit_cubic(p0: Point, p1: Point, p2: Point, p3: Point) -> Euler {
    let d10 = p1 - p0;
    let d32 = p3 - p2;
    let d30 = p3 - p0;

    let th0 = d30.cross(d10).atan2(d30.dot(d10));
    let th1 = d32.cross(d30).atan2(d32.dot(d30));
    let th = d30.y.atan2(d30.x);

    euler_fit(p0, d30.length() as f64, th as f64, -th0 as f64, -th1 as f64)
}
