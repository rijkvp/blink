use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    IO(String),
    Config(String),
    Custom(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(msg) => write!(f, "I/O error: {}", msg),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::Custom(msg) => write!(f, "{}", msg),
        }
    }
}