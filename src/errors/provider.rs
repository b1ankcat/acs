use std::fmt;

#[derive(Debug)]
pub enum ProviderError {
    NotFound { name: String, tool: String },
    NoProviders { tool: String },
    NoRemovable { tool: String },
    InvalidName(String),
}

impl ProviderError {
    pub fn not_found(name: impl Into<String>, tool: impl Into<String>) -> Self {
        Self::NotFound {
            name: name.into(),
            tool: tool.into(),
        }
    }

    pub fn no_providers(tool: impl Into<String>) -> Self {
        Self::NoProviders { tool: tool.into() }
    }

    pub fn no_removable(tool: impl Into<String>) -> Self {
        Self::NoRemovable { tool: tool.into() }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { name, tool } => {
                write!(f, "provider \"{name}\" not found for {tool}")
            }
            Self::NoProviders { tool } => {
                write!(f, "no providers configured for {tool}")
            }
            Self::NoRemovable { tool } => {
                write!(
                    f,
                    "no removable providers for {tool}. the active provider cannot be removed; switch to another first"
                )
            }
            Self::InvalidName(name) => {
                write!(f, "provider name must not be empty or contain '/', '\\', '..': \"{name}\"")
            }
        }
    }
}

impl std::error::Error for ProviderError {}
