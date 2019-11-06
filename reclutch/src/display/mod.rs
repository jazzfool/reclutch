//! Generic high-level vector graphics interface

use palette::Srgba;

pub type Point = euclid::Point2D<f32, euclid::UnknownUnit>;
pub type Vector = euclid::Vector2D<f32, euclid::UnknownUnit>;
pub type Size = euclid::Size2D<f32, euclid::UnknownUnit>;
pub type Rect = euclid::Rect<f32, euclid::UnknownUnit>;
pub type Angle = euclid::Angle<f32>;

/// A trait to process display commands.
///
/// In a retained implementation, command groups are persistent in the underlying graphics API (e.g. vertex buffer objects in OpenGL).
/// Contrasting this, an immediate implementation treats command groups as an instantaneous representation of the scene within `present`.
pub trait GraphicsDisplay {
    /// Resizes the underlying surface.
    fn resize(&mut self, size: (u32, u32));

    /// Pushes a new command group to the scene, returning the handle which can be used to manipulate it later.
    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
    ) -> Result<CommandGroupHandle, failure::Error>;
    /// Returns an existing command group by the handle returned from `push_command_group`.
    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>>;
    /// Overwrites an existing command group by the handle returned from `push_command_group`.
    fn modify_command_group(&mut self, handle: CommandGroupHandle, commands: &[DisplayCommand]);
    /// Removes a command group by the handle returned from `push_command_group`.
    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>>;

    /// Executes pre-exit routines.
    ///
    /// In a GPU implementation, for example, this may wait for the device to finish any remaining draw calls.
    fn before_exit(&mut self);

    /// Displays the entire scene, optionally with a cull.
    fn present(&mut self, cull: Option<Rect>);
}

/// Pushes or modifies a command group, depending on whether `handle` contains a value or not.
/// This means that if `handle` did not contain a value, `push_command_group` will be called and `handle` will be assigned to the returned handle.
pub fn ok_or_push(
    handle: &mut Option<CommandGroupHandle>,
    display: &mut dyn GraphicsDisplay,
    commands: &[DisplayCommand],
) {
    match handle {
        Some(ref handle) => {
            display.modify_command_group(*handle, commands);
        }
        None => {
            *handle = display.push_command_group(commands).ok();
        }
    }
}

/// Handle to a command group within a `GraphicsDisplay`.
#[derive(Debug, Clone, Copy)]
pub struct CommandGroupHandle(u64);

impl CommandGroupHandle {
    /// Creates a new `CommandGroupHandle`, with the inner ID set to `id`.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner ID.
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Stroke cap (stroke start/end) appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    /// The cap of the stroke will appear as expected.
    Flat,
    /// The cap of the stroke will extend tangentially with dimensions square to the stroke width.
    Square,
    /// The end of the stroke will extend tangentially, with a semi-circle.
    Round,
}

/// Path corner appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// The corner will appear as expected.
    Miter,
    /// The corner will be rounded off.
    Round,
    /// The corner will be cut off with a line normal to the mid-value of the tangents of the adjacent lines.
    Bevel,
}

/// Stroke/outline appearance.
#[derive(Clone)]
pub struct GraphicsDisplayStroke {
    /// The color of the stroke.
    pub color: StyleColor,
    /// How thick the stroke should appear; the stroke width.
    pub thickness: f32,
    /// Appearance of the start of the stroke.
    pub begin_cap: LineCap,
    /// Appearance of the end of the stroke.
    pub end_cap: LineCap,
    /// Appearance of the corners of the stroke.
    pub join: LineJoin,
    /// With regards to `LineJoin::Miter`, describes the maximum value of the miter length (the distance between the outer-most and inner-most part of the corner).
    pub miter_limit: f32,
}

/// Appearance of a display item.
#[derive(Clone)]
pub enum GraphicsDisplayPaint {
    /// The item will simply be a color, image, or gradient.
    Fill(StyleColor),
    /// The item will be stroked/outlined.
    Stroke(GraphicsDisplayStroke),
}

/// Describes all the possible graphical items (excluding text, see `TextDisplayItem`).
#[derive(Clone)]
pub enum GraphicsDisplayItem {
    Line {
        /// First point of line.
        a: Point,
        /// Second point of line.
        b: Point,
        /// Stroke of line.
        stroke: GraphicsDisplayStroke,
    },
    Rectangle {
        /// Rectangle coordinates.
        rect: Rect,
        /// Paint style rectangle.
        paint: GraphicsDisplayPaint,
    },
    RoundRectangle {
        /// Rectangle coordinates.
        rect: Rect,
        /// Corner radii of rectangle (from top-left, top-right, bottom-left, bottom-right).
        radii: [f32; 4],
        /// Paint style of rectangle.
        paint: GraphicsDisplayPaint,
    },
    Ellipse {
        /// Center point of ellipse.
        center: Point,
        /// Horizontal/vertical radii of ellipse.
        radii: Vector,
        /// Paint style of ellipse.
        paint: GraphicsDisplayPaint,
    },
}

impl GraphicsDisplayItem {
    /// Returns the inexact maximum boundaries for the item.
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

/// Describes a text render item.
#[derive(Debug, Clone)]
pub struct TextDisplayItem {
    pub text: String,
    pub font: FontInfo,
    pub size: f32,
    pub bottom_left: Point,
    pub color: StyleColor,
}

impl TextDisplayItem {
    /// Returns the exact maximum boundaries for the text.
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

/// Represents a single font.
#[derive(Debug, Clone)]
pub struct FontInfo {
    name: String,
    pub font: font_kit::font::Font,
}

impl FontInfo {
    /// Creates a new font reference, matched to the font `name`, with optional `fallbacks`.
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

    /// Returns the final unique name of the loaded font.
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

/// An item that can be displayed.
#[derive(Clone)]
pub enum DisplayItem {
    /// Graphical item; anything that isn't text.
    Graphics(GraphicsDisplayItem),
    /// Render-able text item.
    Text(TextDisplayItem),
}

impl DisplayItem {
    /// Returns maximum boundaries for the item.
    pub fn bounds(&self) -> Result<Rect, failure::Error> {
        match self {
            DisplayItem::Graphics(item) => Ok(item.bounds()),
            DisplayItem::Text(text) => Ok(text.bounds()?),
        }
    }
}

/// Describes all possible display commands.
#[derive(Clone)]
pub enum DisplayCommand {
    /// Display an item
    Item(DisplayItem),
    /// Applies a filter onto the frame with a mask.
    BackdropFilter(GraphicsDisplayItem, Filter),
    /// Pushes a clip onto the draw state.
    PushClip(GraphicsDisplayItem),
    /// Removes a clip from the draw state.
    PopClip,
    /// Saves the draw state (clip and transformations).
    Save,
    /// Restores a last saved draw state.
    Restore,
    /// Adds translation to the transformation matrix.
    Translate(Vector),
    /// Adds scaling (stretching) to the transformation matrix.
    Scale(Vector),
    /// Adds rotation to the transformation matrix.
    Rotate(Angle),
}

impl DisplayCommand {
    /// Returns the maximum bounds.
    /// Somewhat unorthodox function, since most variants aren't directly graphically expressible.
    pub fn bounds(&self) -> Result<Option<Rect>, failure::Error> {
        Ok(match self {
            DisplayCommand::Item(item) => Some(item.bounds()?),
            DisplayCommand::BackdropFilter(item, _) => Some(item.bounds()),
            DisplayCommand::PushClip(clip) => Some(clip.bounds()),
            _ => None,
        })
    }
}

/// Returns the total maximum for a list of display commands.
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

/// Interpolation between multiple colors.
#[derive(Debug, Clone)]
pub struct Gradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<(f32, Color)>,
}

pub type Color = Srgba;

/// Possible ways to paint a stroke/fill.
#[derive(Debug, Clone)]
pub enum StyleColor {
    /// Solid color.
    Color(Color),
    /// Linear gradient (simply from point A to B).
    LinearGradient(Gradient),
    /// Radial gradient (center being point A and point B being the edge of the circle).
    RadialGradient(Gradient),
}

impl StyleColor {
    /// Returns solid color if possible, otherwise black.
    pub fn color_or_black(&self) -> Color {
        match self {
            StyleColor::Color(color) => *color,
            _ => Color::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// Graphical filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    Blur(f32, f32),
    Invert,
}
