#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct fxp6(pub i32);
impl fxp6 {
    pub fn new(x: i32) -> Self {
        Self(x)
    }

    pub fn from_f32(x: f32) -> Self {
        Self(unsafe { (x * 64.0).to_int_unchecked() })
    }

    pub fn i32(self) -> i32 {
        self.0 >> 6
    }

    pub fn f32(self) -> f32 {
        self.0 as f32 * (1.0 / 64.0)
    }

    pub fn f64(self) -> f64 {
        self.0 as f64 * (1.0 / 64.0)
    }

    pub fn trunc(self) -> Self {
        Self(self.0 & !63)
    }

    pub fn fract(self) -> Self {
        Self(self.0 & 63)
    }
}
