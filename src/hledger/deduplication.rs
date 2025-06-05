use crate::config::HledgerConfig;
use crate::error::Result;
use std::collections::HashSet;
use std::process::Command;

pub fn get_hledger_codes(
    config: &HledgerConfig,
    accounts: HashSet<String>,
) -> Result<HashSet<String>> {
    let mut args: Vec<String> = vec!["codes".to_owned()];
    accounts.into_iter().for_each(|account| args.push(account));

    let output = Command::new(&config.path).args(args).output()?;
    let codes = std::str::from_utf8(&output.stdout)?;
    let result = codes.lines().map(|c| c.to_string()).collect();
    Ok(result)
}
