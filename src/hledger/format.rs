use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::{config::HledgerConfig, error::*};

pub fn hledger_format(config: &HledgerConfig, transactions: &str) -> Result<String> {
    let mut process = Command::new(&config.path)
        .arg("print")
        .arg("-x")
        .arg("-f-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(ImportError::HledgerExecution)?;

    if let Some(mut stdin) = process.stdin.take() {
        stdin
            .write_all(transactions.as_bytes())
            .map_err(ImportError::HledgerExecution)?;
    }

    let mut output = String::new();
    if let Some(mut stdout) = process.stdout.take() {
        stdout
            .read_to_string(&mut output)
            .map_err(ImportError::HledgerExecution)?;
    }

    process.wait().map_err(ImportError::HledgerExecution)?;

    Ok(output)
}
