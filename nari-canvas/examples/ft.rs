use libc::{self, c_long, c_void, size_t};

use nari_ochre::Rasterizer;
use std::{pin::Pin, ptr};
use zeno::{PathBuilder, Point};

fn main() -> anyhow::Result<()> {
    let library = Library::new();
    let fira_code = library.create_font(std::fs::read("assets/arial.ttf")?);
    fira_code.scale(18);
    fira_code.load_char('A');

    unsafe {
        dbg!(&*(*fira_code.face).size);
        dbg!(fira_code.properties());
        dbg!((*(*fira_code.face).glyph).metrics);
    }

    let mut rasterizer = Rasterizer::default();
    fira_code.outline(&mut rasterizer);

    Ok(())
}
