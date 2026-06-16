use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(String),
    Api(String),
    Config(String),
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Database(msg) => write!(f, "Database error: {}", msg),
            Error::Api(msg) => write!(f, "API error: {}", msg),
            Error::Config(msg) => write!(f, "Config error: {}", msg),
            Error::Io(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
