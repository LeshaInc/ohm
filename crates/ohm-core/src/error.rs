use std::error::Error as StdError;
use std::fmt::{self, Debug, Display};

/// An alias for [`Result<T>`](std::result::Result) with [`Error`] as the error
/// type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A list of various error categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A generic error that doesn't fall under any other category.
    Other,

    /// Failed to allocate space in a texture atlas.
    AtlasAlloc,
    /// A cached failure, which means an error has already been reported in a
    /// previous operation.
    CachedFailure,
    /// A generic error caused by GPU related code (graphics API's, video
    /// drivers, lack of resources or unsupported features, etc).
    Gpu,
    /// Failed to parse a font.
    InvalidFont,
    /// Failed to parse an image.
    InvalidImage,
    /// Provided path is invalid (e.g. doesn't match the format or points
    /// somewhere wrong).
    InvalidPath,
    /// A generic IO error.
    Io,
}

/// A general purpose error type.
pub struct Error {
    repr: Box<Repr>,
}

struct Repr {
    kind: ErrorKind,
    message: String,
    source: Option<Box<dyn StdError + Send>>,
}

impl Error {
    /// Creates an [`Error`] with the provided [`ErrorKind`] and a text message.
    pub fn new<T: Display>(kind: ErrorKind, message: T) -> Error {
        Error {
            repr: Box::new(Repr {
                kind,
                message: message.to_string(),
                source: None,
            }),
        }
    }

    /// Wraps a foreign error into this type, additionally providing an
    /// [`ErrorKind`] for it.
    pub fn wrap<E: StdError + Send + 'static>(kind: ErrorKind, source: E) -> Error {
        Error::new(kind, source.to_string()).with_source(source)
    }

    /// Specifies a source error for this one.
    pub fn with_source<E: StdError + Send + 'static>(mut self, source: E) -> Error {
        self.repr.source = Some(Box::new(source));
        self
    }

    /// Creates a new error, which has the same [`ErrorKind`] as `self`, `self`
    /// as source, but a different message.
    ///
    /// This is intended for providing additional context, for example path to a
    /// file which caused an error.
    pub fn with_context<T: Display>(self, context: T) -> Error {
        Error {
            repr: Box::new(Repr {
                kind: self.repr.kind,
                message: context.to_string(),
                source: Some(Box::new(self)),
            }),
        }
    }

    /// Returns the corresponding [`ErrorKind`] for this error.
    pub fn kind(&self) -> ErrorKind {
        self.repr.kind
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.repr.message)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr.source {
            Some(source) => {
                write!(f, "{}, caused by: {:?}", self.repr.message, source)
            }
            None => {
                write!(f, "{}", self.repr.message)
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.repr.source.as_ref().map(|v| (&**v) as &dyn StdError)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::wrap(ErrorKind::Io, error)
    }
}
