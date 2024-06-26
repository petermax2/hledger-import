use std::{fmt::Display, str::Utf8Error};

#[derive(Debug)]
pub enum ImportError {
    HledgerExection(std::io::Error),
    StringConversion(Utf8Error),
    ConfigPath,
    ConfigRead(std::path::PathBuf),
    ConfigParse(toml::de::Error),
    InputFileRead(std::path::PathBuf),
    InputParse(String),
    NumerConversion(String),
    Regex(String),
    Query(String),
    MissingConfig(String),
}

impl Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ImportError::HledgerExection(e) => write!(f, "Failed to interact with hledger: {}",e),
            ImportError::StringConversion(e) => write!(
                f,
                "Encoding/Conversion error while parsing hledger transaction code list output: {}",
                e
            ),
            ImportError::ConfigPath => write!(f, "Failed to provide the path to the configruation file. Please provide the path to the configuration file in the environment variable \"HLEDGER_IMPORT_CONFIG\" to fix this error."),
            ImportError::ConfigRead(path) => write!(f, "Failed to read configuration file \"{}\"", path.to_string_lossy()),
            ImportError::ConfigParse(e) => write!(f, "Failed to parse configuration file: {}", e),
            ImportError::InputFileRead(path) => write!(f, "Failed to read input file \"{}\"", path.to_string_lossy()),
            ImportError::InputParse(msg) => write!(f, "Failed to parse input file: {}", msg),
            ImportError::NumerConversion(txt) => write!(f, "Can not interpret \"{}\" as number (amount)", txt),
            ImportError::Regex(e) => write!(f, "Configuration error in regular expression: {}", e),
            ImportError::Query(e) => write!(f, "Failed to extract transaction information from hledger: {}", e),
            ImportError::MissingConfig(section) => write!(f, "Missing section \"{}\" in configuration", section),
        }
    }
}

impl From<lopdf::Error> for ImportError {
    fn from(value: lopdf::Error) -> Self {
        Self::InputParse(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ImportError>;
