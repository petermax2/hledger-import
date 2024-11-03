use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Failed to interact with hledger: {0}")]
    HledgerExection(#[from] std::io::Error),
    #[error("Encoding or conversion error: {0}")]
    StringConversion(#[from] std::str::Utf8Error),
    #[error("Failed to provide the path to the configruation file. Please provide the path to the configuration file in the environment variable \"HLEDGER_IMPORT_CONFIG\" to fix this error.")]
    ConfigPath,
    #[error("Failed to read configuration file \"{0}\"")]
    ConfigRead(std::path::PathBuf),
    #[error("Failed to parse configuration file: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Failed to read input file \"{0}\"")]
    InputFileRead(std::path::PathBuf),
    #[error("Failed to parse input file: {0}")]
    InputParse(String),
    #[cfg(feature = "flatex")]
    #[error("Failed to parse input PDF file: {0}")]
    PdfInputParse(#[from] lopdf::Error),
    #[error("Can not interpret input as a number: {0}")]
    NumerConversion(String),
    #[error("Configuration error in regular expression: {0}")]
    Regex(#[from] regex::Error),
    #[error("Failed to extract transaction information from hledger: {0}")]
    Query(String),
    #[error("Missing section \"{0}\" in configuration")]
    MissingConfig(String),
    #[error("Missing value \"{0}\" in document")]
    MissingValue(String),
}

pub type Result<T> = std::result::Result<T, ImportError>;
