use std::process::Command;

use crate::{
    config::PopplerConfig,
    error::{ImportError, Result},
};
use tempfile::{self, NamedTempFile};

pub fn extract_text_from_pdf(
    config: &PopplerConfig,
    input_file: &std::path::Path,
) -> Result<String> {
    let tempfile = NamedTempFile::new().map_err(ImportError::TemporaryFileCreationError)?;

    let mut process = Command::new(&config.path)
        .arg("-q")
        .arg("-layout")
        .arg(input_file)
        .arg(tempfile.path())
        .spawn()
        .map_err(ImportError::PopplerProcessError)?;

    process.wait().map_err(ImportError::PopplerProcessError)?;

    let content =
        std::fs::read_to_string(tempfile.path()).map_err(ImportError::TemporaryFileReadError)?;

    Ok(content)
}
