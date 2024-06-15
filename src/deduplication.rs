use crate::error::ImportError;
use crate::error::Result;
use std::collections::HashSet;
use std::process::Command;

pub fn get_hledger_codes() -> Result<HashSet<String>> {
    // TODO take hledger path from configuration
    let output = Command::new("hledger").arg("codes").output();
    let output = match output {
        Ok(o) => o,
        Err(e) => return Err(ImportError::HledgerExection(e)),
    };

    let codes = match std::str::from_utf8(&output.stdout) {
        Ok(c) => c,
        Err(e) => return Err(ImportError::StringConversion(e)),
    };

    let result = codes.lines().map(|c| c.to_string()).collect();
    Ok(result)
}
