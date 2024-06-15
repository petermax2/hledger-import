use std::str::Utf8Error;

pub enum ImportError {
    HledgerExection(std::io::Error),
    StringConversion(Utf8Error),
}

pub type Result<T> = std::result::Result<T, ImportError>;
