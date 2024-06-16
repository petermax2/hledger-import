use std::{fmt::Display, str::Utf8Error};

#[derive(Debug)]
pub enum ImportError {
    HledgerExection(std::io::Error),
    StringConversion(Utf8Error),
    ConfigPath,
    ConfigRead(std::path::PathBuf),
    ConfigParse(toml::de::Error),
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
            ImportError::ConfigPath => write!(f, "Failed to provide the path to the configruation file. Please provide the path to the configuration file in the environment variable \"HLEDGER_IMPORT_CONFIG\" to fix this error."),
            ImportError::ConfigRead(path) => write!(f, "Failed to read configuration file \"{}\"", path.to_string_lossy()),
            ImportError::ConfigParse(e) => write!(f, "Failed to parse configuration file: {}", e),
        }
    }
}

pub type Result<T> = std::result::Result<T, ImportError>;
