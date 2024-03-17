use crate::{
    kurbo::{Affine, BezPath, Rect, Shape, Stroke},
    peniko::{Brush, Fill},
    Scene,
};
use usvg::TreeParsing;

#[derive(Default)]
pub struct IconPath {
    path: BezPath,
    transform: Affine,
    fill: bool,
    stroke: Option<f32>,
}

#[derive(Default)]
pub struct Icon {
    pub bbox: Rect,
    paths: Vec<IconPath>,
}

impl Icon {
    pub fn build(data: &[u8]) -> anyhow::Result<Self> {
        use usvg::NodeExt;

        let mut icon = Icon::default();

        let tree = usvg::Tree::from_data(&data, &usvg::Options::default())?;
        let mut bbox = None;

        for node in tree.root.descendants() {
            if let usvg::NodeKind::Path(upath) = &*node.borrow() {
                let transform = {
                    let usvg::Transform { a, b, c, d, e, f } = node.abs_transform();
                    Affine::new([a, b, c, d, e, f])
                };

                let mut path = BezPath::new();
                for node in upath.data.segments() {
                    match node {
                        usvg::PathSegment::MoveTo { x, y } => path.move_to((x, y)),
                        usvg::PathSegment::LineTo { x, y } => path.line_to((x, y)),
                        usvg::PathSegment::CurveTo {
                            x1,
                            y1,
                            x2,
                            y2,
                            x,
                            y,
                        } => path.curve_to((x1, y1), (x2, y2), (x, y)),
                        usvg::PathSegment::ClosePath => path.close_path(),
                    }
                }

                let bbox_local = path.bounding_box();
                bbox = bbox.map_or(Some(bbox_local), |bbox: Rect| Some(bbox.union(bbox_local)));

                icon.paths.push(IconPath {
                    path,
                    transform,
                    fill: upath.fill.is_some(),
                    stroke: upath.stroke.as_ref().map(|s| s.width.get() as f32),
                });
            }
        }

        if let Some(bbox) = bbox {
            icon.bbox = bbox;
        }

        Ok(icon)
    }

    pub fn paint(&self, sb: &mut Scene, affine: Affine, brush: &Brush) {
        for path in &self.paths {
            let transform = affine * path.transform;

            if path.fill {
                sb.fill(Fill::NonZero, transform, brush, None, &path.path);
            }
            if let Some(stroke) = path.stroke {
                sb.stroke(
                    &Stroke::new(stroke.into()),
                    transform,
                    brush,
                    None,
                    &path.path,
                );
            }
        }
    }
}
