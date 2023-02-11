#[derive(Debug, Clone, Copy)]
pub enum Align {
    Negative,
    Center,
    Positive,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Margin {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> u32 {
        assert!(self.x0 <= self.x1);
        (self.x1 - self.x0) as _
    }

    pub fn height(self) -> u32 {
        assert!(self.y0 <= self.y1);
        (self.y1 - self.y0) as _
    }

    pub fn margin(self, margin: Margin) -> Self {
        // TODO: ensure within bounds?
        let x0 = self.x0 + margin.left;
        let x1 = self.x1 - margin.right;
        let y0 = self.y0 + margin.top;
        let y1 = self.y1 - margin.bottom;

        Self {
            x0: x0.min(self.x1),
            y0: y0.min(self.y1),
            x1: x1.max(self.x0),
            y1: y1.max(self.y0),
        }
    }

    pub fn center(self, rect: Self) -> Self {
        let dx = self.x0 - rect.x0 + self.x1;
        let dy = self.x0 - rect.x0 + self.x1;

        Self {
            x0: (dx - rect.x1) / 2,
            x1: (dx + rect.x1) / 2,
            y0: (dy - rect.y1) / 2,
            y1: (dy + rect.y1) / 2,
        }
    }

    pub fn offset(self, x: i32, y: i32) -> Self {
        Self {
            x0: self.x0 + x,
            x1: self.x1 + x,
            y0: self.y0 + y,
            y1: self.y1 + y,
        }
    }

    pub fn hittest(&self, x: i32, y: i32) -> bool {
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }
}
