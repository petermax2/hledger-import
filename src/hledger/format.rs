use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::{config::HledgerConfig, error::*};

pub fn hledger_format(
    config: &HledgerConfig,
    transactions: &str,
    commodity_formatting_rules: &Option<Vec<String>>,
) -> Result<String> {
    let args: Vec<&str> = if let Some(rules) = commodity_formatting_rules {
        dbg!(rules);
        let mut args = vec!["print", "-x", "-f-", "--round=soft"];
        rules.iter().for_each(|r| {
            args.push("-c");
            args.push(r);
        });
        args
    } else {
        dbg!("no formatting rules here :-( ");
        vec!["print", "-x", "-f-"]
    };
    dbg!(&args);

    let mut process = Command::new(&config.path)
        .args(args)
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
