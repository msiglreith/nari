use parley::layout::{Cursor, Layout};
use parley::style::Brush;
use std::ops::Range;

struct Paragraph {
    text: String,
}

struct ParagraphList {
    paragraph: Paragraph,
    next: Option<usize>,
    prev: Option<usize>,
}

#[derive(Default)]
pub struct Notebook {
    paragraphs: Vec<ParagraphList>,
}

pub enum Erase {
    None,
    Full(Range<usize>),
}

pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
}

impl Selection {
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let focus = Cursor::from_point(layout, x, y);
        dbg!(focus.path);
        Self {
            anchor: focus,
            focus,
        }
    }

    pub fn from_cursor(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            focus: cursor,
        }
    }

    pub fn normalized_range(&self) -> Range<usize> {
        let anchor_offset = if self.anchor.is_leading() {
            self.anchor.text_start
        } else {
            self.anchor.text_end
        };
        let focus_offset = if self.focus.is_leading() {
            self.focus.text_start
        } else {
            self.focus.text_end
        };

        if focus_offset < anchor_offset {
            focus_offset..anchor_offset
        } else {
            anchor_offset..focus_offset
        }
    }

    pub fn is_collapsed(&self) -> bool {
        (self.anchor.path, self.anchor.is_leading()) == (self.focus.path, self.focus.is_leading())
    }

    pub fn op_backspace(&self) -> Erase {
        if !self.is_collapsed() {
            return Erase::Full(self.normalized_range());
        }

        todo!()
    }
}
