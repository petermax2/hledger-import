use std::{fmt::Display, str::Utf8Error};

#[derive(Debug)]
pub enum ImportError {
    HledgerExection(std::io::Error),
    StringConversion(Utf8Error),
}

impl Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ImportError::HledgerExection(e) => write!(
                f,
                "Failed to read list of known transaction codes from hledger: {}",
                e
            ),
            ImportError::StringConversion(e) => write!(
                f,
                "Encoding/Conversion error while parsing hledger transaction code list output: {}",
                e
            ),
        }
    }
}

pub type Result<T> = std::result::Result<T, ImportError>;
