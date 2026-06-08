use std::fmt;

#[derive(Debug)]
pub enum InteractiveError {
    Input(String),
    Cancelled,
}

impl InteractiveError {
    pub fn input(message: impl Into<String>) -> Self {
        Self::Input(message.into())
    }
}

impl fmt::Display for InteractiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(msg) => write!(f, "input error: {msg}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for InteractiveError {}

impl From<dialoguer::Error> for InteractiveError {
    fn from(e: dialoguer::Error) -> Self {
        Self::Input(e.to_string())
    }
}
