//! Generic high-level vector graphics interface

use palette::Srgba;

pub type Point = euclid::Point2D<f32, euclid::UnknownUnit>;
pub type Vector = euclid::Vector2D<f32, euclid::UnknownUnit>;
pub type Size = euclid::Size2D<f32, euclid::UnknownUnit>;
pub type Rect = euclid::Rect<f32, euclid::UnknownUnit>;
pub type Angle = euclid::Angle<f32>;

pub trait GraphicsDisplay {
    fn resize(&mut self, size: (u32, u32));

    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
    ) -> Result<CommandGroupHandle, failure::Error>;
    fn get_command_group(&self, handle: &CommandGroupHandle) -> Option<Vec<DisplayCommand>>;
    fn modify_command_group(&mut self, handle: &CommandGroupHandle, commands: &[DisplayCommand]);
    fn remove_command_group(&mut self, handle: &CommandGroupHandle) -> Option<Vec<DisplayCommand>>;

    fn before_exit(&mut self);

    fn present(&mut self, cull: Option<Rect>);
}

pub fn ok_or_push(
    handle: &mut Option<CommandGroupHandle>,
    display: &mut dyn GraphicsDisplay,
    commands: &[DisplayCommand],
) {
    match handle {
        Some(ref handle) => {
            display.modify_command_group(handle, commands);
        }
        None => {
            if let Ok(h) = display.push_command_group(commands) {
                *handle = Some(h);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandGroupHandle(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Flat,
    Square,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone)]
pub struct GraphicsDisplayStroke {
    pub color: StyleColor,
    pub thickness: f32,
    pub begin_cap: LineCap,
    pub end_cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}

#[derive(Clone)]
pub enum GraphicsDisplayPaint {
    Fill(StyleColor),
    Stroke(GraphicsDisplayStroke),
}

/// Describes all the possible draw commands (excluding text, see `TextDisplayItem`).
#[derive(Clone)]
pub enum GraphicsDisplayItem {
    Line {
        /// First point of line
        a: Point,
        /// Second point of line
        b: Point,
        /// Stroke of line
        stroke: GraphicsDisplayStroke,
    },
    Rectangle {
        /// Rectangle coordinates
        rect: Rect,
        /// Paint style rectangle
        paint: GraphicsDisplayPaint,
    },
    RoundRectangle {
        /// Rectangle coordinates
        rect: Rect,
        /// Corner radii of rectangle (from top-left, top-right, bottom-left, bottom-right)
        radii: [f32; 4],
        /// Paint style of rectangle
        paint: GraphicsDisplayPaint,
    },
    Ellipse {
        /// Center point of ellipse
        center: Point,
        /// Horizontal/vertical radii of ellipse
        radii: Vector,
        /// Paint style of ellipse
        paint: GraphicsDisplayPaint,
    },
}

impl GraphicsDisplayItem {
    pub fn bounds(&self) -> Rect {
        match self {
            GraphicsDisplayItem::Line { a, b, stroke } => {
                Rect::from_points([*a, *b].iter()).inflate(stroke.thickness, stroke.thickness)
            }
            GraphicsDisplayItem::Rectangle { rect, paint } => match paint {
                GraphicsDisplayPaint::Fill(_) => *rect,
                GraphicsDisplayPaint::Stroke(stroke) => {
                    rect.inflate(stroke.thickness, stroke.thickness)
                }
            },
            GraphicsDisplayItem::RoundRectangle { rect, paint, .. } => match paint {
                GraphicsDisplayPaint::Fill(_) => *rect,
                GraphicsDisplayPaint::Stroke(stroke) => {
                    rect.inflate(stroke.thickness, stroke.thickness)
                }
            },
            GraphicsDisplayItem::Ellipse {
                center,
                radii,
                paint,
            } => {
                let rect = Rect::new(
                    (center.x - radii.x, center.y - radii.y).into(),
                    (radii.x * 2.0, radii.y * 2.0).into(),
                );
                match paint {
                    GraphicsDisplayPaint::Fill(_) => rect,
                    GraphicsDisplayPaint::Stroke(stroke) => {
                        rect.inflate(stroke.thickness, stroke.thickness)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextDisplayItem {
    pub text: String,
    pub font: FontInfo,
    pub size: f32,
    pub bottom_left: Point,
    pub color: StyleColor,
}

impl TextDisplayItem {
    pub fn bounds(&self) -> Result<Rect, failure::Error> {
        let mut rect = Rect::new(self.bottom_left, Size::default());

        for c in self.text.chars() {
            rect = rect.union(
                &self
                    .font
                    .font
                    .raster_bounds(
                        self.font.font.glyph_for_char(c).unwrap_or_default(),
                        self.size,
                        &font_kit::loader::FontTransform::identity(),
                        &self.bottom_left,
                        font_kit::hinting::HintingOptions::Full(self.size),
                        font_kit::canvas::RasterizationOptions::SubpixelAa,
                    )?
                    .to_f32(),
            );
        }

        Ok(rect)
    }
}

#[derive(Debug, Clone)]
pub struct FontInfo {
    name: String,
    font: font_kit::font::Font,
}

impl FontInfo {
    pub fn new(name: &str, fallbacks: &[&str]) -> Result<Self, failure::Error> {
        let mut names = vec![font_kit::family_name::FamilyName::Title(name.to_string())];
        names.append(
            &mut fallbacks
                .iter()
                .map(|&s| font_kit::family_name::FamilyName::Title(s.to_string()))
                .collect::<Vec<_>>(),
        );

        let font = font_kit::source::SystemSource::new()
            .select_best_match(&names, &font_kit::properties::Properties::default())
            .unwrap()
            .load()
            .unwrap();
        Ok(Self {
            name: font.full_name(),
            font,
        })
    }
}

#[derive(Clone)]
pub enum DisplayItem {
    Graphics(GraphicsDisplayItem),
    Text(TextDisplayItem),
}

impl DisplayItem {
    pub fn bounds(&self) -> Result<Rect, failure::Error> {
        match self {
            DisplayItem::Graphics(item) => Ok(item.bounds()),
            DisplayItem::Text(text) => Ok(text.bounds()?),
        }
    }
}

#[derive(Clone)]
pub enum DisplayCommand {
    /// Display an item
    Item(DisplayItem),
    /// Applies a filter onto the frame with a mask
    BackdropFilter(GraphicsDisplayItem, Filter),
    /// Pushes a clip onto the draw state
    PushClip(GraphicsDisplayItem),
    /// Removes a clip from the draw state
    PopClip,
    /// Saves the draw state (clip and tranformations)
    Save,
    /// Restores a last saved draw state
    Restore,
    /// Adds translation to the transformation matrix
    Translate(Vector),
    /// Adds scaling (stretching) to the transformation matrix
    Scale(Vector),
    /// Adds rotation to the transformation matrix
    Rotate(Angle),
}

impl DisplayCommand {
    pub fn bounds(&self) -> Result<Option<Rect>, failure::Error> {
        Ok(if let DisplayCommand::Item(item) = self {
            Some(item.bounds()?)
        } else {
            None
        })
    }
}

pub fn display_list_bounds(display_list: &[DisplayCommand]) -> Result<Rect, failure::Error> {
    Ok(display_list
        .iter()
        .filter_map(|disp| {
            if let DisplayCommand::Item(item) = disp {
                Some(item.bounds())
            } else {
                None
            }
        })
        .try_fold::<Option<Rect>, _, Result<_, failure::Error>>(None, |rect, bounds| {
            let bounds = bounds?;
            Ok(Some(rect.map_or(bounds, |rc| rc.union(&bounds))))
        })?
        .unwrap_or_default())
}

#[derive(Debug, Clone)]
pub struct Gradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<(f32, Color)>,
}

pub type Color = Srgba;

#[derive(Debug, Clone)]
pub enum StyleColor {
    Color(Color),
    LinearGradient(Gradient),
    RadialGradient(Gradient),
}

impl StyleColor {
    pub fn color_or_black(&self) -> Color {
        match self {
            StyleColor::Color(color) => *color,
            _ => Color::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    Blur(f32, f32),
}
