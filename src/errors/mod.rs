mod config;
mod import_;
mod interactive;
mod provider;

pub use config::ConfigError;
pub use import_::ImportError;
pub use interactive::InteractiveError;
pub use provider::ProviderError;

use std::fmt;

#[derive(Debug)]
pub enum AcsError {
    Config(ConfigError),
    Provider(ProviderError),
    Import(ImportError),
    Interactive(InteractiveError),
}

impl fmt::Display for AcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(e) => write!(f, "config error: {e}"),
            Self::Provider(e) => write!(f, "provider error: {e}"),
            Self::Import(e) => write!(f, "import error: {e}"),
            Self::Interactive(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::Provider(e) => Some(e),
            Self::Import(e) => Some(e),
            Self::Interactive(e) => Some(e),
        }
    }
}

impl From<ConfigError> for AcsError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}

impl From<ProviderError> for AcsError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

impl From<ImportError> for AcsError {
    fn from(e: ImportError) -> Self {
        Self::Import(e)
    }
}

impl From<InteractiveError> for AcsError {
    fn from(e: InteractiveError) -> Self {
        Self::Interactive(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::load("/tmp/test", io::Error::new(io::ErrorKind::NotFound, "no file"));
        let display = err.to_string();
        assert!(display.contains("failed to read"));
        assert!(display.contains("/tmp/test"));
        assert!(display.contains("no file"));
    }

    #[test]
    fn test_config_error_source() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ConfigError::save("/tmp/x", io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::NotFound {
            name: "my-prov".into(),
            tool: "claude".into(),
        };
        let display = err.to_string();
        assert!(display.contains("my-prov"));
        assert!(display.contains("claude"));
    }

    #[test]
    fn test_import_error_display() {
        let err = ImportError::parse("/tmp/bad.toml", "invalid TOML");
        let display = err.to_string();
        assert!(display.contains("failed to parse"));
        assert!(display.contains("/tmp/bad.toml"));
    }

    #[test]
    fn test_interactive_error_display() {
        let err = InteractiveError::Cancelled;
        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn test_acp_error_wraps_sub_errors() {
        let config_err = ConfigError::serialize("test error");
        let acp_err = AcsError::from(config_err);
        let display = acp_err.to_string();
        assert!(display.contains("config error"));
        assert!(acp_err.source().is_some());
    }

    #[test]
    fn test_from_impl_conversions() {
        let err: AcsError = ConfigError::serialize("test error").into();
        assert!(matches!(err, AcsError::Config(_)));

        let err: AcsError = ProviderError::InvalidName("bad".into()).into();
        assert!(matches!(err, AcsError::Provider(_)));

        let err: AcsError = ImportError::Read {
            path: "/tmp/x".into(),
            source: io::Error::new(io::ErrorKind::NotFound, "gone"),
        }
        .into();
        assert!(matches!(err, AcsError::Import(_)));

        let err: AcsError = InteractiveError::Cancelled.into();
        assert!(matches!(err, AcsError::Interactive(_)));
    }
}
