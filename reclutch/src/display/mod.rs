//! Generic high-level vector graphics interface

#[cfg(feature = "skia")]
pub mod skia;

#[cfg(feature = "gpu")]
pub mod gpu;

use {crate::error, palette::Srgba, std::sync::Arc};

/// Two-dimensional floating-point absolute point.
pub type Point = euclid::Point2D<f32, euclid::UnknownUnit>;
/// Two-dimensional floating-point relative vector.
pub type Vector = euclid::Vector2D<f32, euclid::UnknownUnit>;
/// Two-dimensional floating-point size.
pub type Size = euclid::Size2D<f32, euclid::UnknownUnit>;
/// Two-dimensional floating-point rectangle.
pub type Rect = euclid::Rect<f32, euclid::UnknownUnit>;
/// An angle in radians.
pub type Angle = euclid::Angle<f32>;

/// A trait to process display commands.
///
/// In a retained implementation, command groups are persistent in the underlying graphics API (e.g. vertex buffer objects in OpenGL).
/// Contrasting this, an immediate implementation treats command groups as an instantaneous representation of the scene within [`present`](trait.GraphicsDisplay.html#tymethod.present).
pub trait GraphicsDisplay {
    /// Resizes the underlying surface.
    fn resize(&mut self, size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>>;

    /// Creates a new resource for use in rendering.
    fn new_resource(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceReference, error::ResourceError>;
    /// Removes an existing resource.
    fn remove_resource(&mut self, reference: ResourceReference);

    /// Pushes a new command group to the scene, returning the handle which can be used to manipulate it later.
    ///
    /// Normally [`Save`](enum.DisplayCommand.html#variant.Save) and [`Restore`](enum.DisplayCommand.html#variant.Restore) (more specifically an internal `RestoreToCount`) is invoked between command group execution to prevent any leaking
    /// of clips/transforms, however this can be explicitly disabled by letting `protected` be `false`.
    fn push_command_group(
        &mut self,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    ) -> Result<CommandGroupHandle, Box<dyn std::error::Error>>;
    /// Returns an existing command group by the handle returned from [`push_command_group`](trait.GraphicsDisplay.html#tymethod.push_command_group).
    fn get_command_group(&self, handle: CommandGroupHandle) -> Option<&[DisplayCommand]>;
    /// Overwrites an existing command group by the handle returned from [`push_command_group`](trait.GraphicsDisplay.html#tymethod.push_command_group).
    fn modify_command_group(
        &mut self,
        handle: CommandGroupHandle,
        commands: &[DisplayCommand],
        protected: Option<bool>,
    );
    /// Refreshes a command group.
    /// Typically this means moving the command group to the front.
    fn maintain_command_group(&mut self, handle: CommandGroupHandle);
    /// Removes a command group by the handle returned from [`push_command_group`](trait.GraphicsDisplay.html#tymethod.push_command_group).
    fn remove_command_group(&mut self, handle: CommandGroupHandle) -> Option<Vec<DisplayCommand>>;

    /// Executes pre-exit routines.
    ///
    /// In a GPU implementation, for example, this may wait for the device to finish any remaining draw calls.
    fn before_exit(&mut self);

    /// Displays the entire scene, optionally with a cull.
    fn present(&mut self, cull: Option<Rect>) -> Result<(), error::DisplayError>;
}

/// Resource data, either as a file or an in-memory buffer.
#[derive(Debug, Clone)]
pub enum ResourceData {
    File(std::path::PathBuf),
    Data(SharedData),
}

/// Whether the given image data is encoded.
/// Formats like PNG and JPEG are encoded, however formats like RAW and a simple array of pixels aren't.
#[derive(Debug, Clone)]
pub enum ImageData {
    Encoded(ResourceData),
    Raw(ResourceData, RasterImageInfo),
}

/// How pixels are stored in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterImageFormat {
    /// 4x8-bit components, in order of; red, green, blue and alpha.
    Rgba8,
    /// 4x8-bit components, in order of; blue, green, red and alpha.
    Bgra8,
}

/// Information about a raster image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterImageInfo {
    pub size: (u32, u32),
    pub format: RasterImageFormat,
}

/// Contains information required to load a resource through [`new_resource`](trait.GraphicsDisplay.html#tymethod.new_resource).
#[derive(Debug, Clone)]
pub enum ResourceDescriptor {
    Image(ImageData),
    Font(ResourceData),
}

/// Contains a tagged ID to an existing resource, created through [`new_resource`](trait.GraphicsDisplay.html#tymethod.new_resource).
///
/// This is used to references resources in draw commands and to remove resources through [`remove_resource`](trait.GraphicsDisplay.html#tymethod.remove_resource).
#[derive(Debug, Clone)]
pub enum ResourceReference {
    Image(u64),
    Font(u64),
}

impl ResourceReference {
    /// Returns the inner ID of the resource reference.
    pub fn id(&self) -> u64 {
        match self {
            ResourceReference::Image(id) | ResourceReference::Font(id) => *id,
        }
    }
}

/// Data stored as bytes, either in a atomically reference counted `Vec` or a static reference.
#[derive(Debug, Clone)]
pub enum SharedData {
    RefCount(Arc<Vec<u8>>),
    Static(&'static [u8]),
}

/// Pushes or modifies a command group, depending on whether `handle` contains a value or not.
/// This means that if `handle` did not contain a value, [`push_command_group`](trait.GraphicsDisplay.html#tymethod.push_command_group) will be called and `handle` will be assigned to the returned handle.
pub fn ok_or_push(
    handle: &mut Option<CommandGroupHandle>,
    display: &mut dyn GraphicsDisplay,
    commands: &[DisplayCommand],
    protected: impl Into<Option<bool>>,
) {
    match handle {
        Some(ref handle) => {
            display.modify_command_group(*handle, commands, protected.into());
        }
        None => {
            *handle = Some(
                display
                    .push_command_group(commands, protected.into())
                    .unwrap(),
            );
            //*handle = display.push_command_group(commands, protected.into()).ok();
        }
    }
}

/// Handle to a command group within a [`GraphicsDisplay`](trait.GraphicsDisplay.html).
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

/// Helper wrapper around [`CommandGroupHandle`](struct.CommandGroupHandle.html).
#[derive(Clone)]
pub struct CommandGroup(Option<CommandGroupHandle>, bool);

impl CommandGroup {
    /// Creates a new, empty command group.
    pub fn new() -> Self {
        CommandGroup(None, true)
    }

    /// Pushes a list of commands if the repaint flag is set, and resets repaint flag if so.
    ///
    /// See [`push_command_group`](trait.GraphicsDisplay.html#tymethod.push_command_group)
    pub fn push(
        &mut self,
        display: &mut dyn GraphicsDisplay,
        commands: &[DisplayCommand],
        protected: impl Into<Option<bool>>,
    ) {
        if self.1 {
            self.1 = false;
            ok_or_push(&mut self.0, display, commands, protected);
        } else {
            display.maintain_command_group(self.0.unwrap());
        }
    }

    /// Sets the repaint flag so that next time [`push`](struct.CommandGroup.html#tymethod.push) is called the commands will be pushed.
    pub fn repaint(&mut self) {
        self.1 = true;
    }

    /// Returns flag indicating whether next [`push`](struct.CommandGroup.html#tymethod.push) will skip or not.
    pub fn will_repaint(&self) -> bool {
        self.1
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

impl Default for LineCap {
    fn default() -> Self {
        LineCap::Flat
    }
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

impl Default for LineJoin {
    fn default() -> Self {
        LineJoin::Miter
    }
}

/// Stroke/outline appearance.
#[derive(Clone)]
pub struct GraphicsDisplayStroke {
    /// The color of the stroke.
    pub color: StyleColor,
    /// How thick the stroke should appear; the stroke width.
    pub thickness: f32,
    /// Appearance of the caps of the stroke.
    pub cap: LineCap,
    /// Appearance of the corners of the stroke.
    pub join: LineJoin,
    /// With regards to [`miter`](enum.LineJoin.html#variant.Miter), describes the maximum value of the miter length (the distance between the outer-most and inner-most part of the corner).
    pub miter_limit: f32,
    /// Whether this stroke should be antialiased or not. This can be used to achieve sharp, thin outlines.
    pub antialias: bool,
}

impl Default for GraphicsDisplayStroke {
    fn default() -> Self {
        GraphicsDisplayStroke {
            color: StyleColor::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            thickness: 1.0,
            cap: LineCap::default(),
            join: LineJoin::default(),
            miter_limit: 4.0,
            antialias: true,
        }
    }
}

/// Appearance of a display item.
#[derive(Clone)]
pub enum GraphicsDisplayPaint {
    /// The item will simply be a color, image, or gradient.
    Fill(StyleColor),
    /// The item will be stroked/outlined.
    Stroke(GraphicsDisplayStroke),
}

/// Describes all the possible graphical items (excluding text, see [`TextDisplayItem`](struct.TextDisplayItem.html)).
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
    Image {
        /// Optional source sample rectangle.
        src: Option<Rect>,
        /// Destination output rectangle.
        dst: Rect,
        /// Reference to the image resource.
        resource: ResourceReference,
    },
}

impl GraphicsDisplayItem {
    /// Returns the exact maximum boundaries for the item.
    pub fn bounds(&self) -> Rect {
        match self {
            GraphicsDisplayItem::Line { a, b, stroke } => {
                let size = Size::new(1.0, (*a - *b).length());
                let axis_rect_xy =
                    Point::new((a.x + b.x) / 2.0, ((a.y + b.y) / 2.0) - (size.height / 2.0));
                rotated_rectangle_bounds(
                    &Rect::new(axis_rect_xy, size).inflate(
                        stroke.thickness / 2.0,
                        if stroke.cap != LineCap::Flat {
                            stroke.thickness / 2.0
                        } else {
                            0.0
                        },
                    ),
                    &Angle::radians(2.0 * ((*a - axis_rect_xy).length() / size.height).asin()),
                )
            }
            GraphicsDisplayItem::Rectangle { rect, paint } => match paint {
                GraphicsDisplayPaint::Fill(_) => *rect,
                GraphicsDisplayPaint::Stroke(stroke) => {
                    rect.inflate(stroke.thickness / 2.0, stroke.thickness / 2.0)
                }
            },
            GraphicsDisplayItem::RoundRectangle { rect, paint, .. } => match paint {
                GraphicsDisplayPaint::Fill(_) => *rect,
                GraphicsDisplayPaint::Stroke(stroke) => {
                    rect.inflate(stroke.thickness / 2.0, stroke.thickness / 2.0)
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
                        rect.inflate(stroke.thickness / 2.0, stroke.thickness / 2.0)
                    }
                }
            }
            GraphicsDisplayItem::Image { dst, .. } => dst.clone(),
        }
    }
}

/// A single shaped glyph.
/// This should be generated from the output of a shaping engine.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub codepoint: u32,
    pub advance: Vector,
    pub offset: Vector,
}

/// Render-able text, either as a simple string or pre-shaped glyphs (via a library such as HarfBuzz).
#[derive(Debug, Clone)]
pub enum DisplayText {
    Simple(String),
    Shaped(Vec<ShapedGlyph>),
}

impl From<String> for DisplayText {
    fn from(text: String) -> Self {
        DisplayText::Simple(text)
    }
}

/// Describes a text render item.
#[derive(Debug, Clone)]
pub struct TextDisplayItem {
    pub text: DisplayText,
    pub font: ResourceReference,
    pub font_info: FontInfo,
    pub size: f32,
    pub bottom_left: Point,
    pub color: StyleColor,
}

impl TextDisplayItem {
    /// Returns the maximum boundaries for the text.
    ///
    /// The height of the bounding box is conservative; it doesn't change based on the contents of
    /// [`text`](struct.TextDisplayItem.html#structfield.text), is defined on a per-font basis, and is "worst-case" (as in it represents the largest
    /// height value in the font).
    ///
    /// The bounding box is identical to that of a browser's.
    pub fn bounds(&self) -> Result<Rect, error::FontError> {
        let metrics = self.font_info.font.metrics();
        let units_per_em = metrics.units_per_em as f32;

        let font_height = metrics.ascent - metrics.descent;
        let line_height = if font_height > units_per_em {
            font_height
        } else {
            font_height + metrics.line_gap
        };
        let height = line_height / units_per_em * self.size;

        let y = self.bottom_left.y - metrics.ascent / units_per_em * self.size;

        let width = match self.text {
            DisplayText::Simple(ref text) => {
                text.as_bytes().iter().try_fold(
                    0.0,
                    |width, &character| -> Result<f32, error::FontError> {
                        Ok(width
                            + self
                                .font_info
                                .font
                                .advance(
                                    self.font_info
                                        .font
                                        .glyph_for_char(character as char)
                                        .ok_or(error::FontError::CodepointError)?,
                                )?
                                .x)
                    },
                )? / units_per_em
                    * self.size
            }
            DisplayText::Shaped(ref glyphs) => glyphs
                .iter()
                .fold(0.0, |width, glyph| width + glyph.advance.x),
        };

        Ok(
            Rect::new(Point::new(self.bottom_left.x, y), Size::new(width, height))
                .inflate(self.size / 8.0, 0.0),
        )
    }
}

/// Represents a single font.
#[derive(Debug, Clone)]
pub struct FontInfo {
    name: String,
    /// Underlying font reference.
    pub font: Arc<font_kit::font::Font>,
}

impl FontInfo {
    /// Creates a new font reference, matched to the font `name`, with optional `fallbacks`.
    pub fn from_name(name: &str, fallbacks: &[&str]) -> Result<Self, error::FontError> {
        let mut names = vec![font_kit::family_name::FamilyName::Title(name.to_string())];
        names.append(
            &mut fallbacks
                .iter()
                .map(|&s| font_kit::family_name::FamilyName::Title(s.to_string()))
                .collect::<Vec<_>>(),
        );

        let font = font_kit::source::SystemSource::new()
            .select_best_match(&names, &font_kit::properties::Properties::default())?
            .load()?;

        Ok(Self {
            name: font.full_name(),
            font: Arc::new(font),
        })
    }

    /// Creates a new font reference from a font file located at `path`.
    ///
    /// If the font file contains more than one font, use `font_index` to select the font to load.
    pub fn from_path<P: AsRef<std::path::Path>>(
        path: P,
        font_index: u32,
    ) -> Result<Self, error::FontError> {
        let font = font_kit::font::Font::from_path(path, font_index)?;

        Ok(Self {
            name: font.full_name(),
            font: Arc::new(font),
        })
    }

    /// Creates a new font reference from font data.
    /// Similar to [`from_path`](struct.FontInfo.html#tymethod.from_path), however as bytes rather than a path to a file.
    pub fn from_data(data: Arc<Vec<u8>>, font_index: u32) -> Result<Self, error::FontError> {
        let font = font_kit::font::Font::from_bytes(data, font_index)?;

        Ok(Self {
            name: font.full_name(),
            font: Arc::new(font),
        })
    }

    /// Returns the final unique name of the loaded font.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Returns the font data as bytes.
    pub fn data(&self) -> Option<Vec<u8>> {
        Some((*self.font.copy_font_data()?).clone())
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
    pub fn bounds(&self) -> Result<Rect, error::FontError> {
        match self {
            DisplayItem::Graphics(item) => Ok(item.bounds()),
            DisplayItem::Text(text) => Ok(text.bounds()?),
        }
    }
}

/// Clipping shapes.
#[derive(Debug, Clone, Copy)]
pub enum DisplayClip {
    /// Rectangle clip.
    Rectangle {
        rect: Rect,
        /// As a general rule, set to true if [`rect`](enum.DisplayClip.html#variant.Rectangle.field.rect) isn't pixel-aligned.
        antialias: bool,
    },
    RoundRectangle {
        rect: Rect,
        radii: [f32; 4],
    },
    Ellipse {
        center: Point,
        radii: Vector,
    },
}

impl DisplayClip {
    pub fn bounds(&self) -> Rect {
        match self {
            DisplayClip::Rectangle { ref rect, .. }
            | DisplayClip::RoundRectangle { ref rect, .. } => (*rect),
            DisplayClip::Ellipse {
                ref center,
                ref radii,
            } => Rect::new(
                (center.x - radii.x, center.y - radii.y).into(),
                (radii.x * 2.0, radii.y * 2.0).into(),
            ),
        }
    }
}

/// Describes all possible display commands.
#[derive(Clone)]
pub enum DisplayCommand {
    /// Display an item
    Item(DisplayItem),
    /// Applies a filter onto the frame with a mask.
    BackdropFilter(DisplayClip, Filter),
    /// Pushes a clip onto the draw state.
    /// To remove the clip, call this after a [`save`](enum.DisplayCommand.html#variant.Save) command, which once [`restored`](enum.DisplayCommand.html#variant.Restore), the clip will be removed.
    Clip(DisplayClip),
    /// Saves the draw state (clip and transformations).
    Save,
    /// Saves the draw state (clip and transformations) and begins drawing into a new layer.
    /// The float value is the layer opacity.
    SaveLayer(f32),
    /// Restores a last saved draw state.
    Restore,
    /// Adds translation to the transformation matrix.
    Translate(Vector),
    /// Adds scaling (stretching) to the transformation matrix.
    Scale(Vector),
    /// Adds rotation to the transformation matrix.
    Rotate(Angle),
    /// Fills the clipped region with a solid color.
    Clear(Color),
}

impl DisplayCommand {
    /// Returns the maximum bounds.
    /// Somewhat unorthodox function, since most variants aren't directly graphically expressible.
    pub fn bounds(&self) -> Result<Option<Rect>, error::FontError> {
        Ok(match self {
            DisplayCommand::Item(item) => Some(item.bounds()?),
            DisplayCommand::BackdropFilter(item, _) => Some(item.bounds()),
            DisplayCommand::Clip(clip) => Some(clip.bounds()),
            _ => None,
        })
    }
}

/// Returns the total maximum for a list of display commands.
pub fn display_list_bounds(display_list: &[DisplayCommand]) -> Result<Rect, error::FontError> {
    Ok(display_list
        .iter()
        .filter_map(|disp| {
            if let DisplayCommand::Item(item) = disp {
                Some(item.bounds())
            } else {
                None
            }
        })
        .try_fold::<Option<Rect>, _, Result<_, error::FontError>>(None, |rect, bounds| {
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

impl From<Color> for StyleColor {
    fn from(color: Color) -> Self {
        StyleColor::Color(color)
    }
}

/// Graphical filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    Blur(f32, f32),
    Invert,
}

/// Interface to simplify creating a list of display commands.
#[derive(Clone)]
pub struct DisplayListBuilder {
    display_list: Vec<DisplayCommand>,
}

impl DisplayListBuilder {
    /// Creates a new, empty display list builder.
    pub fn new() -> Self {
        DisplayListBuilder {
            display_list: Vec::new(),
        }
    }

    /// Creates a new display list builder, initialized with an existing list of display commands.
    pub fn from_commands(commands: &[DisplayCommand]) -> Self {
        DisplayListBuilder {
            display_list: commands.to_vec(),
        }
    }

    /// Pushes a stroked line, spanning from `a` to `b`.
    pub fn push_line(&mut self, a: Point, b: Point, stroke: GraphicsDisplayStroke) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Graphics(
                GraphicsDisplayItem::Line { a, b, stroke },
            )));
    }

    /// Pushes a filled/stroked rectangle.
    pub fn push_rectangle(&mut self, rect: Rect, paint: GraphicsDisplayPaint) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Graphics(
                GraphicsDisplayItem::Rectangle { rect, paint },
            )));
    }

    /// Pushes a filled/stroked rectangle, with rounded corners.
    pub fn push_round_rectangle(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        paint: GraphicsDisplayPaint,
    ) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Graphics(
                GraphicsDisplayItem::RoundRectangle { rect, radii, paint },
            )));
    }

    /// Pushes a filled/stroked ellipse.
    pub fn push_ellipse(&mut self, center: Point, radii: Vector, paint: GraphicsDisplayPaint) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Graphics(
                GraphicsDisplayItem::Ellipse {
                    center,
                    radii,
                    paint,
                },
            )));
    }

    /// Pushes an image.
    pub fn push_image(
        &mut self,
        src: impl Into<Option<Rect>>,
        dst: Rect,
        image: ResourceReference,
    ) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Graphics(
                GraphicsDisplayItem::Image {
                    src: src.into(),
                    dst,
                    resource: image,
                },
            )));
    }

    /// Pushes a line of text.
    pub fn push_text(&mut self, text: TextDisplayItem) {
        self.display_list
            .push(DisplayCommand::Item(DisplayItem::Text(text)));
    }

    /// Pushes a rectangle which applies a filter on everything behind it.
    pub fn push_rectangle_backdrop(&mut self, rect: Rect, antialias: bool, filter: Filter) {
        self.display_list.push(DisplayCommand::BackdropFilter(
            DisplayClip::Rectangle { rect, antialias },
            filter,
        ));
    }

    /// Pushes a rectangle with rounded corners which applies a filter on everything behind it.
    pub fn push_round_rectangle_backdrop(&mut self, rect: Rect, radii: [f32; 4], filter: Filter) {
        self.display_list.push(DisplayCommand::BackdropFilter(
            DisplayClip::RoundRectangle { rect, radii },
            filter,
        ));
    }

    /// Pushes an ellipse which applies a filter on everything behind it.
    pub fn push_ellipse_backdrop(&mut self, center: Point, radii: Vector, filter: Filter) {
        self.display_list.push(DisplayCommand::BackdropFilter(
            DisplayClip::Ellipse { center, radii },
            filter,
        ));
    }

    /// Pushes a rectangle which clips proceeding display commands.
    pub fn push_rectangle_clip(&mut self, rect: Rect, antialias: bool) {
        self.display_list
            .push(DisplayCommand::Clip(DisplayClip::Rectangle {
                rect,
                antialias,
            }));
    }

    /// Pushes a rectangle with rounded corners which clips proceeding display commands.
    pub fn push_round_rectangle_clip(&mut self, rect: Rect, radii: [f32; 4]) {
        self.display_list
            .push(DisplayCommand::Clip(DisplayClip::RoundRectangle {
                rect,
                radii,
            }));
    }

    /// Pushes an ellipse which clips proceeding display commands.
    pub fn push_ellipse_clip(&mut self, center: Point, radii: Vector) {
        self.display_list
            .push(DisplayCommand::Clip(DisplayClip::Ellipse { center, radii }));
    }

    /// Saves the current draw state (clip, transformation, layers).
    pub fn save(&mut self) {
        self.display_list.push(DisplayCommand::Save);
    }

    /// Saves the current draw state (clip, transformation, layers) and begins drawing to a new layer, with a specified opacity.
    pub fn save_layer(&mut self, opacity: f32) {
        self.display_list.push(DisplayCommand::SaveLayer(opacity));
    }

    /// Restores previously saved states.
    pub fn restore(&mut self) {
        self.display_list.push(DisplayCommand::Restore);
    }

    /// Pushes translation (offset) to the transformation matrix.
    pub fn push_translation(&mut self, translation: Vector) {
        self.display_list
            .push(DisplayCommand::Translate(translation));
    }

    /// Pushes scaling to the transformation matrix.
    pub fn push_scaling(&mut self, scaling: Vector) {
        self.display_list.push(DisplayCommand::Scale(scaling));
    }

    /// Pushes rotation to the transformation matrix.
    pub fn push_rotation(&mut self, rotation: Angle) {
        self.display_list.push(DisplayCommand::Rotate(rotation));
    }

    /// Fills the screen/clip with a solid color.
    pub fn push_clear(&mut self, color: Color) {
        self.display_list.push(DisplayCommand::Clear(color));
    }

    /// Returns the final list of display commands.
    pub fn build(self) -> Vec<DisplayCommand> {
        self.display_list
    }
}

fn rotate_point(p: &Point, center: &Point, angle: &Angle) -> Point {
    let (angle_sin, angle_cos) = angle.sin_cos();
    Point::new(
        angle_cos * (p.x - center.x) - angle_sin * (p.y - center.y) + center.x,
        angle_sin * (p.x - center.x) + angle_cos * (p.y - center.y) + center.y,
    )
}

fn rotated_rectangle_bounds(rect: &Rect, angle: &Angle) -> Rect {
    Rect::from_points(
        [
            rect.origin,
            rect.origin + rect.size,
            rect.origin + Size::new(rect.size.width, 0.0),
            rect.origin + Size::new(0.0, rect.size.height),
        ]
        .iter()
        .map(|p| rotate_point(p, &rect.center(), angle)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use float_cmp::approx_eq;

    fn epsilon_rect(a: &Rect, b: &Rect) {
        assert!(approx_eq!(f32, a.origin.x, b.origin.x, epsilon = TOLERANCE));
        assert!(approx_eq!(f32, a.origin.y, b.origin.y, epsilon = TOLERANCE));
        assert!(approx_eq!(
            f32,
            a.size.width,
            b.size.width,
            epsilon = TOLERANCE
        ));
        assert!(approx_eq!(
            f32,
            a.size.height,
            b.size.height,
            epsilon = TOLERANCE
        ));
    }

    // Tolerance for what is determined to be a correct boundary.
    const TOLERANCE: f32 = 1.0;

    #[test]
    fn test_line_bounds() {
        epsilon_rect(
            &GraphicsDisplayItem::Line {
                a: Point::new(64.0, 32.0),
                b: Point::new(128.0, 64.0),
                stroke: GraphicsDisplayStroke {
                    thickness: 16.0,
                    ..Default::default()
                },
            }
            .bounds(),
            &Rect::new(Point::new(60.0, 24.0), Size::new(71.0, 47.0)),
        );
    }

    #[test]
    fn test_rectangle_fill_bounds() {
        const RECT: Rect = Rect::new(Point::new(-20.0, 70.0), Size::new(15.0, 50.0));
        epsilon_rect(
            &GraphicsDisplayItem::Rectangle {
                rect: RECT,
                paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::default())),
            }
            .bounds(),
            &RECT,
        );
    }

    #[test]
    fn test_rectangle_stroke_bounds() {
        epsilon_rect(
            &GraphicsDisplayItem::Rectangle {
                rect: Rect::new(Point::new(-20.0, 70.0), Size::new(15.0, 50.0)),
                paint: GraphicsDisplayPaint::Stroke(GraphicsDisplayStroke {
                    thickness: 8.0,
                    ..Default::default()
                }),
            }
            .bounds(),
            &Rect::new(Point::new(-24.0, 66.0), Size::new(23.0, 58.0)),
        );
    }

    #[test]
    fn test_round_rectangle_fill_bounds() {
        const RECT: Rect = Rect::new(Point::new(-20.0, 70.0), Size::new(15.0, 50.0));
        epsilon_rect(
            &GraphicsDisplayItem::RoundRectangle {
                rect: RECT,
                radii: [10.0; 4],
                paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::default())),
            }
            .bounds(),
            &RECT,
        );
    }

    #[test]
    fn test_round_rectangle_stroke_bounds() {
        epsilon_rect(
            &GraphicsDisplayItem::RoundRectangle {
                rect: Rect::new(Point::new(-20.0, 70.0), Size::new(15.0, 50.0)),
                radii: [10.0; 4],
                paint: GraphicsDisplayPaint::Stroke(GraphicsDisplayStroke {
                    thickness: 8.0,
                    ..Default::default()
                }),
            }
            .bounds(),
            &Rect::new(Point::new(-24.0, 66.0), Size::new(23.0, 58.0)),
        );
    }

    #[test]
    fn test_ellipse_fill_bounds() {
        epsilon_rect(
            &GraphicsDisplayItem::Ellipse {
                center: Point::new(13.0, -56.0),
                radii: Vector::new(43.0, 12.0),
                paint: GraphicsDisplayPaint::Fill(StyleColor::Color(Color::default())),
            }
            .bounds(),
            &Rect::new(Point::new(-30.0, -68.0), Size::new(86.0, 24.0)),
        );
    }

    #[test]
    fn test_ellipse_stroke_bounds() {
        epsilon_rect(
            &GraphicsDisplayItem::Ellipse {
                center: Point::new(13.0, -56.0),
                radii: Vector::new(43.0, 12.0),
                paint: GraphicsDisplayPaint::Stroke(GraphicsDisplayStroke {
                    thickness: 8.0,
                    ..Default::default()
                }),
            }
            .bounds(),
            &Rect::new(Point::new(-34.0, -72.0), Size::new(94.0, 32.0)),
        );
    }
}
