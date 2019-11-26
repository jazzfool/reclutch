use thiserror::Error;

/// An error within `font_kit`.
#[derive(Error, Debug)]
pub enum FontError {
    #[error("{0}")]
    LoadingError(#[from] font_kit::error::FontLoadingError),
    #[error("{0}")]
    GlyphLoadingError(#[from] font_kit::error::GlyphLoadingError),
    #[error("{0}")]
    MatchingError(#[from] font_kit::error::SelectionError),
    #[error("failed to look up matching codepoint for character")]
    CodepointError,
}

/// An error within Skia and its interactions with OpenGL.
#[derive(Error, Debug)]
#[cfg(feature = "skia")]
pub enum SkiaError {
    #[error("the OpenGL target {0} is invalid")]
    InvalidTarget(String),
    #[error("invalid OpenGL context")]
    InvalidContext,
    #[error("unknown skia error")]
    UnknownError,
}

/// An error within the GPU graphics display implementation.
#[derive(Error, Debug)]
#[cfg(feature = "gpu")]
pub enum GpuError {
    #[error("shaderc error: {0}")]
    CompilerError(String),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("failed to get GPU adapter")]
    AdapterError,
    #[error("failed to tessellate mesh")]
    TessellationError,
    #[error("{0}")]
    FontError(#[from] FontError),
}

/// An error associated with loading graphical resources.
#[derive(Error, Debug)]
pub enum ResourceError {
    #[error("{0} is not a file")]
    InvalidPath(String),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("given resource data is invalid and cannot be read/decoded")]
    InvalidData,
    #[error("{0}")]
    InternalError(#[from] Box<dyn std::error::Error>),
}

/// An error related to [`GraphicsDisplay`](../display/trait.GraphicsDisplay.html).
#[derive(Error, Debug)]
pub enum DisplayError {
    #[error("{0}")]
    ResourceError(#[from] ResourceError),
    #[error("non-existent resource reference (id: {0})")]
    InvalidResource(u64),
    #[error("mismatched resource reference type (id: {0})")]
    MismatchedResource(u64),
    #[error("{0}")]
    InternalError(#[from] Box<dyn std::error::Error>),
    #[error("{0}")]
    FontError(#[from] FontError),
}
