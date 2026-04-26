#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization: {0}")]
    Serialization(String),
    #[error("image: {0}")]
    Image(String),
    #[error("output format is required when writing to stdout")]
    FormatRequired,
    #[error("chrome connection: {0}")]
    ChromeConnection(String),
    #[error("chrome timeout")]
    ChromeTimeout,
    #[error("mermaid syntax: {0}")]
    MermaidSyntax(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",
            Self::InvalidArgument(_) => "INVALID_ARGUMENT",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Timeout(_) => "TIMEOUT",
            Self::Io(_) => "IO_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::Image(_) => "IMAGE_ERROR",
            Self::FormatRequired => "FORMAT_REQUIRED",
            Self::ChromeConnection(_) => "CHROME_CONNECTION",
            Self::ChromeTimeout => "CHROME_TIMEOUT",
            Self::MermaidSyntax(_) => "MERMAID_SYNTAX",
            Self::Internal(_) => "INTERNAL",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_stable() {
        assert_eq!(Error::UnsupportedFormat("x".into()).code(), "UNSUPPORTED_FORMAT");
        assert_eq!(Error::InvalidArgument("x".into()).code(), "INVALID_ARGUMENT");
        assert_eq!(Error::NotFound("x".into()).code(), "NOT_FOUND");
        assert_eq!(Error::Timeout(100).code(), "TIMEOUT");
        assert_eq!(Error::Serialization("x".into()).code(), "SERIALIZATION_ERROR");
        assert_eq!(Error::Image("x".into()).code(), "IMAGE_ERROR");
        assert_eq!(Error::FormatRequired.code(), "FORMAT_REQUIRED");
        assert_eq!(Error::ChromeConnection("x".into()).code(), "CHROME_CONNECTION");
        assert_eq!(Error::ChromeTimeout.code(), "CHROME_TIMEOUT");
        assert_eq!(Error::MermaidSyntax("x".into()).code(), "MERMAID_SYNTAX");
        assert_eq!(Error::Internal("x".into()).code(), "INTERNAL");
    }

    #[test]
    fn io_error_converts() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let err: Error = io.into();
        assert_eq!(err.code(), "IO_ERROR");
    }
}
