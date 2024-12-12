use crate::config::HledgerConfig;
use crate::error::ImportError;
use crate::error::Result;
use std::collections::HashSet;
use std::process::Command;

pub fn get_hledger_codes(config: &HledgerConfig) -> Result<HashSet<String>> {
    let output = Command::new(&config.path).arg("codes").output();
    let output = match output {
        Ok(o) => o,
        Err(e) => return Err(ImportError::HledgerExecution(e)),
    };

    let codes = match std::str::from_utf8(&output.stdout) {
        Ok(c) => c,
        Err(e) => return Err(ImportError::StringConversion(e)),
    };

    let result = codes.lines().map(|c| c.to_string()).collect();
    Ok(result)
}
