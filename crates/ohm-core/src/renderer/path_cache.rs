use std::num::NonZeroUsize;

use glam::Vec4;
use lru::LruCache;
use lyon_tessellation::geom::Point;
use lyon_tessellation::path::PathEvent as LyonPathEvent;
use lyon_tessellation::{
    BuffersBuilder, FillTessellator, FillVertex, StrokeTessellator, StrokeVertex, VertexBuffers,
};

use crate::math::{Rect, Vec2};
use crate::path::{FillOptions, FillRule, LineCap, LineJoin, Path, PathEvent, StrokeOptions};
use crate::renderer::{Vertex, INSTANCE_FILL};

const CAPACITY: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathKey(*const PathEvent);

unsafe impl Send for PathKey {}
unsafe impl Sync for PathKey {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Fill(PathKey, FillOptions),
    Stroke(PathKey, StrokeOptions),
}

#[derive(Debug)]
pub struct Mesh {
    pub bounding_rect: Option<Rect>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct PathCache {
    lru: LruCache<Key, Mesh>,
    stroke_tessellator: StrokeTessellator,
    fill_tessellator: FillTessellator,
}

impl PathCache {
    pub fn new() -> PathCache {
        PathCache {
            lru: LruCache::new(NonZeroUsize::new(CAPACITY).unwrap()),
            stroke_tessellator: StrokeTessellator::new(),
            fill_tessellator: FillTessellator::new(),
        }
    }

    pub fn fill(&mut self, path: &Path, options: &FillOptions) -> &Mesh {
        let path_key = PathKey(path.events().as_ptr() as *const _);
        self.lru.get_or_insert(Key::Fill(path_key, *options), || {
            let mut buffers = VertexBuffers::new();

            let mut output = BuffersBuilder::new(&mut buffers, |vertex: FillVertex<'_>| {
                let pos = Vec2::new(vertex.position().x, vertex.position().y);
                Vertex {
                    pos,
                    local_pos: pos,
                    tex: pos,
                    color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                    instance_id: INSTANCE_FILL,
                }
            });

            self.fill_tessellator
                .tessellate(
                    path_to_events(path),
                    &lyon_fill_options(options),
                    &mut output,
                )
                .expect("failed to fill path");

            Mesh {
                bounding_rect: compute_bounding_rect(&buffers.vertices),
                vertices: buffers.vertices,
                indices: buffers.indices,
            }
        })
    }

    pub fn stroke(&mut self, path: &Path, options: &StrokeOptions) -> &Mesh {
        let path_key = PathKey(path.events().as_ptr() as *const _);
        self.lru.get_or_insert(Key::Stroke(path_key, *options), || {
            let mut buffers = VertexBuffers::new();

            let mut output = BuffersBuilder::new(&mut buffers, |vertex: StrokeVertex<'_, '_>| {
                let pos = Vec2::new(vertex.position().x, vertex.position().y);
                Vertex {
                    pos,
                    local_pos: pos,
                    tex: pos,
                    color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                    instance_id: INSTANCE_FILL,
                }
            });

            self.stroke_tessellator
                .tessellate(
                    path_to_events(path),
                    &lyon_stroke_options(options),
                    &mut output,
                )
                .expect("failed to stroke path");

            Mesh {
                bounding_rect: compute_bounding_rect(&buffers.vertices),
                vertices: buffers.vertices,
                indices: buffers.indices,
            }
        })
    }
}

impl Default for PathCache {
    fn default() -> Self {
        PathCache::new()
    }
}

fn compute_bounding_rect(vertices: &[Vertex]) -> Option<Rect> {
    if vertices.is_empty() {
        return None;
    }

    let mut min = vertices[0].pos;
    let mut max = vertices[0].pos;

    for vertex in &vertices[1..] {
        min = min.min(vertex.pos);
        max = max.max(vertex.pos);
    }

    Some(Rect::new(min, max))
}

fn path_to_events(path: &Path) -> impl Iterator<Item = LyonPathEvent> + '_ {
    let mut events = path.events().iter().peekable();

    let mut closed = true;
    let mut start_pos = Vec2::ZERO;
    let mut cur_pos = Vec2::ZERO;

    std::iter::from_fn(move || {
        let lyon_event;

        match **events.peek()? {
            PathEvent::MoveTo { point } if closed => {
                lyon_event = LyonPathEvent::Begin {
                    at: lyon_point(point),
                };

                cur_pos = point;
                start_pos = point;
                events.next()?;
            }
            PathEvent::MoveTo { .. } => {
                lyon_event = LyonPathEvent::End {
                    last: lyon_point(cur_pos),
                    first: lyon_point(start_pos),
                    close: false,
                };
            }
            PathEvent::LineTo { point } => {
                lyon_event = LyonPathEvent::Line {
                    from: lyon_point(cur_pos),
                    to: lyon_point(point),
                };

                cur_pos = point;
                events.next()?;
            }
            PathEvent::QuadTo { control, point } => {
                lyon_event = LyonPathEvent::Quadratic {
                    from: lyon_point(cur_pos),
                    ctrl: lyon_point(control),
                    to: lyon_point(point),
                };

                cur_pos = point;
                events.next()?;
            }
            PathEvent::CubicTo { control, point } => {
                lyon_event = LyonPathEvent::Cubic {
                    from: lyon_point(cur_pos),
                    ctrl1: lyon_point(control[0]),
                    ctrl2: lyon_point(control[1]),
                    to: lyon_point(point),
                };

                cur_pos = point;
                events.next()?;
            }
            PathEvent::Close => {
                lyon_event = LyonPathEvent::End {
                    last: lyon_point(cur_pos),
                    first: lyon_point(start_pos),
                    close: true,
                };

                closed = true;
                events.next()?;
            }
        };

        Some(lyon_event)
    })
}

fn lyon_fill_options(options: &FillOptions) -> lyon_tessellation::FillOptions {
    lyon_tessellation::FillOptions::DEFAULT.with_fill_rule(lyon_fill_rule(options.fill_rule))
}

fn lyon_fill_rule(rule: FillRule) -> lyon_tessellation::FillRule {
    match rule {
        FillRule::EvenOdd => lyon_tessellation::FillRule::EvenOdd,
        FillRule::NonZero => lyon_tessellation::FillRule::NonZero,
    }
}

fn lyon_stroke_options(options: &StrokeOptions) -> lyon_tessellation::StrokeOptions {
    lyon_tessellation::StrokeOptions::DEFAULT
        .with_line_cap(lyon_line_cap(options.line_cap))
        .with_line_join(lyon_line_join(options.line_join))
        .with_line_width(options.line_width)
        .with_miter_limit(options.mitter_limit)
}

fn lyon_line_cap(cap: LineCap) -> lyon_tessellation::LineCap {
    match cap {
        LineCap::Butt => lyon_tessellation::LineCap::Butt,
        LineCap::Square => lyon_tessellation::LineCap::Square,
        LineCap::Round => lyon_tessellation::LineCap::Round,
    }
}

fn lyon_line_join(join: LineJoin) -> lyon_tessellation::LineJoin {
    match join {
        LineJoin::Miter => lyon_tessellation::LineJoin::Miter,
        LineJoin::MiterClip => lyon_tessellation::LineJoin::MiterClip,
        LineJoin::Round => lyon_tessellation::LineJoin::Round,
        LineJoin::Bevel => lyon_tessellation::LineJoin::Bevel,
    }
}

fn lyon_point(p: Vec2) -> Point<f32> {
    Point::new(p.x, p.y)
}
