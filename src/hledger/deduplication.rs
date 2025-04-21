use crate::config::HledgerConfig;
use crate::error::Result;
use std::collections::HashSet;
use std::process::Command;

pub fn get_hledger_codes(config: &HledgerConfig) -> Result<HashSet<String>> {
    let output = Command::new(&config.path).arg("codes").output()?;
    let codes = std::str::from_utf8(&output.stdout)?;
    let result = codes.lines().map(|c| c.to_string()).collect();
    Ok(result)
}
