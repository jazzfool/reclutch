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
}

/// An error within Skia and its interactions with OpenGL.
#[derive(Error, Debug)]
pub enum SkiaError {
    #[error("the OpenGL target {0} is invalid")]
    InvalidTarget(String),
    #[error("invalid OpenGL context")]
    InvalidContext,
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
