use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ConfigError {
    Load { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    Save { path: PathBuf, source: io::Error },
    Serialize { message: String },
    DirCreate { path: PathBuf, source: io::Error },
    Permissions { path: PathBuf, source: io::Error },
    Remove { path: PathBuf, source: io::Error },
}

impl ConfigError {
    pub fn load(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Load {
            path: path.into(),
            source,
        }
    }

    pub fn parse(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Parse {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn save(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Save {
            path: path.into(),
            source,
        }
    }

    pub fn serialize(message: impl Into<String>) -> Self {
        Self::Serialize {
            message: message.into(),
        }
    }

    pub fn dir_create(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::DirCreate {
            path: path.into(),
            source,
        }
    }

    pub fn permissions(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Permissions {
            path: path.into(),
            source,
        }
    }

    pub fn remove(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Remove {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Save { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
            Self::Serialize { message } => {
                write!(f, "failed to serialize: {message}")
            }
            Self::DirCreate { path, source } => {
                write!(f, "failed to create directory {}: {source}", path.display())
            }
            Self::Permissions { path, source } => {
                write!(
                    f,
                    "failed to set permissions on {}: {source}",
                    path.display()
                )
            }
            Self::Remove { path, source } => {
                write!(f, "failed to remove {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load { source, .. }
            | Self::Save { source, .. }
            | Self::DirCreate { source, .. }
            | Self::Permissions { source, .. }
            | Self::Remove { source, .. } => Some(source),
            _ => None,
        }
    }
}
