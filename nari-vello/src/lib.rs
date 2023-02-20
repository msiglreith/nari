mod engine;
mod fxp;
pub mod typo;

pub use self::engine::Engine;

#[derive(Debug, Clone, Copy)]
pub enum Align {
    Negative,
    Center,
    Positive,
}
