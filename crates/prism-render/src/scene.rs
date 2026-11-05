//! A backend-agnostic description of a drawn graph.
//!
//! Rendering produces a [`Scene`]: a flat, ordered list of primitives in points,
//! with the origin at the top-left of the page. The GUI paints it with `egui`, the
//! exporter turns it into SVG, and both see exactly the same geometry.

use prism_doc::graph::{Color, LineStyle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

pub const fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for Rect {
    fn default() -> Self {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Point {
        pt(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// Shrink by `d` on every side.
    pub fn shrink(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Baseline,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
    pub style: LineStyle,
}

impl Stroke {
    pub fn new(color: Color, width: f32) -> Self {
        Stroke { color, width, style: LineStyle::Solid }
    }

    /// The dash pattern for this style, in points, or `None` when solid.
    pub fn dash_array(&self) -> Option<Vec<f32>> {
        let w = self.width.max(0.5);
        match self.style {
            LineStyle::Solid | LineStyle::None => None,
            LineStyle::Dash => Some(vec![w * 4.0, w * 3.0]),
            LineStyle::Dot => Some(vec![w, w * 2.0]),
            LineStyle::DashDot => Some(vec![w * 4.0, w * 2.0, w, w * 2.0]),
        }
    }
}

impl From<&prism_doc::graph::Stroke> for Stroke {
    fn from(s: &prism_doc::graph::Stroke) -> Self {
        Stroke { color: s.color, width: s.width, style: s.style }
    }
}

#[derive(Debug, Clone)]
pub struct TextRun {
    pub text: String,
    pub at: Point,
    pub size: f32,
    pub family: String,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub h: HAlign,
    pub v: VAlign,
    /// Degrees, counter-clockwise about `at`.
    pub rotation: f32,
}

/// One drawing primitive.
#[derive(Debug, Clone)]
pub enum DrawCmd {
    Line { from: Point, to: Point, stroke: Stroke },
    /// An open or closed sequence of points.
    Polyline { points: Vec<Point>, stroke: Stroke, close: bool },
    Polygon { points: Vec<Point>, fill: Color, stroke: Option<Stroke> },
    Rect { rect: Rect, fill: Color, stroke: Option<Stroke> },
    Circle { center: Point, radius: f32, fill: Color, stroke: Option<Stroke> },
    /// A pie or donut slice. Angles are degrees, counter-clockwise from east.
    Wedge {
        center: Point,
        radius: f32,
        inner_radius: f32,
        start_angle: f32,
        sweep: f32,
        fill: Color,
        stroke: Option<Stroke>,
    },
    Text(TextRun),
}

/// What a primitive belongs to, so the GUI can hit-test and highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Background,
    Axis,
    Grid,
    /// A mark belonging to the series at this index.
    Series(usize),
    Legend,
    Title,
}

#[derive(Debug, Clone)]
pub struct TaggedCmd {
    pub cmd: DrawCmd,
    pub tag: Tag,
}

/// A fully laid-out graph.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// Page size in points.
    pub size: (f32, f32),
    /// The data area, excluding titles, tick labels and legend.
    pub plot_area: Rect,
    pub commands: Vec<TaggedCmd>,
}

impl Scene {
    pub fn push(&mut self, tag: Tag, cmd: DrawCmd) {
        self.commands.push(TaggedCmd { cmd, tag });
    }

    pub fn iter(&self) -> impl Iterator<Item = &TaggedCmd> {
        self.commands.iter()
    }
}
