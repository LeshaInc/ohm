use std::error::Error as StdError;
use std::fmt::{self, Debug, Display};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    Other,

    AtlasAlloc,
    CachedFailure,
    Gpu,
    InvalidFont,
    InvalidId,
    InvalidImage,
    InvalidPath,
    Io,
    UnknownSchema,
}

pub struct Error {
    repr: Box<Repr>,
}

struct Repr {
    kind: ErrorKind,
    message: String,
    source: Option<Box<dyn StdError + Send>>,
}

impl Error {
    pub fn new<T: Display>(kind: ErrorKind, message: T) -> Error {
        Error {
            repr: Box::new(Repr {
                kind,
                message: message.to_string(),
                source: None,
            }),
        }
    }

    pub fn wrap<E: StdError + Send + 'static>(kind: ErrorKind, source: E) -> Error {
        Error::new(kind, source.to_string()).with_source(source)
    }

    pub fn with_source<E: StdError + Send + 'static>(mut self, source: E) -> Error {
        self.repr.source = Some(Box::new(source));
        self
    }

    pub fn with_context<T: Display>(self, context: T) -> Error {
        Error {
            repr: Box::new(Repr {
                kind: self.repr.kind,
                message: context.to_string(),
                source: Some(Box::new(self)),
            }),
        }
    }

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
