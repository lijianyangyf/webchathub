use std::fmt;
use std::io;
use serde_json;
use tungstenite;

#[derive(Debug)]
pub enum ChatError {
    Io(io::Error),
    Serde(serde_json::Error),
    Tungstenite(tungstenite::Error),
    Custom(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::Io(err) => write!(f, "IO Error: {}", err),
            ChatError::Serde(err) => write!(f, "Serde Error: {}", err),
            ChatError::Tungstenite(err) => write!(f, "Tungstenite Error: {}", err),
            ChatError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ChatError {}

impl From<io::Error> for ChatError {
    fn from(err: io::Error) -> Self {
        ChatError::Io(err)
    }
}

impl From<serde_json::Error> for ChatError {
    fn from(err: serde_json::Error) -> Self {
        ChatError::Serde(err)
    }
}

impl From<tungstenite::Error> for ChatError {
    fn from(err: tungstenite::Error) -> Self {
        ChatError::Tungstenite(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_display_io_error() {
        let err = ChatError::Io(io::Error::new(io::ErrorKind::Other, "disk full"));
        assert!(format!("{}", err).contains("disk full"));
    }

    #[test]
    fn test_display_custom_error() {
        let err = ChatError::Custom("my error".into());
        assert_eq!(format!("{}", err), "my error");
    }
}
