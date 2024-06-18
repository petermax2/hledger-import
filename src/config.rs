use crate::error::{ImportError, Result};
use homedir::get_my_home;
use serde::Deserialize;
use std::str::FromStr;

/// encapsulation of the application configuration
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ImporterConfig {
    #[serde(default)]
    pub hledger: HledgerConfig,
    pub ibans: Vec<IbanMapping>,
    pub cards: Vec<CardMapping>,
    pub mapping: Vec<SimpleMapping>,
    pub creditor_and_debitor_mapping: Vec<CreditorDebitorMapping>,
    pub sepa: SepaConfig,
    pub transfer_accounts: TransferAccounts,
}

impl ImporterConfig {
    pub fn path() -> Result<std::path::PathBuf> {
        let env_path = std::env::var("HLEDGER_IMPORT_CONFIG");
        match env_path {
            Ok(env) => match std::path::PathBuf::from_str(&env) {
                Ok(path) => Ok(path),
                Err(_) => Err(ImportError::ConfigPath),
            },
            Err(_) => match get_my_home() {
                Ok(home) => match home {
                    Some(home) => {
                        let mut path = home.into_os_string();
                        path.push("/.config/hledger-import/config.toml");
                        Ok(path.into())
                    }
                    None => Err(ImportError::ConfigPath),
                },
                Err(_) => Err(ImportError::ConfigPath),
            },
        }
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let config_str = std::fs::read_to_string(&path);
        match config_str {
            Ok(config_str) => match toml::from_str::<ImporterConfig>(&config_str) {
                Ok(config) => Ok(config),
                Err(parse_err) => Err(ImportError::ConfigParse(parse_err)),
            },
            Err(_) => Err(ImportError::ConfigRead(path)),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct HledgerConfig {
    pub path: String,
}

impl Default for HledgerConfig {
    fn default() -> Self {
        Self {
            path: "hledger".to_owned(),
        }
    }
}

/// Maps an IBAN to a hleger asset/liability account
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct IbanMapping {
    pub iban: String,
    pub account: String,
    pub fees_account: Option<String>,
    pub note: Option<String>,
}

/// Maps a credit card number (or identifier) to a hleger asset/liability account
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CardMapping {
    pub card: String,
    pub account: String,
    pub fees_account: Option<String>,
    pub note: Option<String>,
}

/// Encapsulates configuration of SEPA-payment identification
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SepaConfig {
    pub creditors: Vec<SepaCreditorMapping>,
    pub mandates: Vec<SepaMandateMapping>,
}

/// Maps SEPA-Mandate ID to hledger account
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SepaMandateMapping {
    pub mandate_id: String,
    pub account: String,
    pub note: Option<String>,
}

/// Maps SEPA-Creditor ID to hledger account
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SepaCreditorMapping {
    pub creditor_id: String,
    pub account: String,
    pub note: Option<String>,
}

/// Definition of the hledger accounts that should be used to post bank transfers and cash transfers
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct TransferAccounts {
    pub bank: String,
    pub cash: String,
}

/// Search for given regular expression and post to account, if the search matches
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SimpleMapping {
    pub search: String,
    pub account: String,
    pub note: Option<String>,
}

/// Search for given regular expression and post to account, if the expression matches.
/// Also create a P/L posting on "default_pl_account", if defined and no matching transaction exists on the P/L account.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CreditorDebitorMapping {
    pub search: String,
    pub account: String,
    pub default_pl_account: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_toml_str() {
        let config_str = "ibans = []
        cards = []
        mapping = []
        creditor_and_debitor_mapping = []

        [sepa]
        creditors = []
        mandates = []
        
        [transfer_accounts]
        bank = \"Assets:Bank\"
        cash = \"Assets:Cash\"

        [hledger]
        path = \"/opt/homebrew/bin/hledger\"
        "
        .to_owned();
        let expected = ImporterConfig {
            hledger: HledgerConfig {
                path: "/opt/homebrew/bin/hledger".to_owned(),
            },
            ibans: vec![],
            cards: vec![],
            mapping: vec![],
            creditor_and_debitor_mapping: vec![],
            sepa: SepaConfig {
                creditors: vec![],
                mandates: vec![],
            },
            transfer_accounts: TransferAccounts {
                bank: "Assets:Bank".to_owned(),
                cash: "Assets:Cash".to_owned(),
            },
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);

        let config_str = "ibans = []
        cards = []
        mapping = []
        creditor_and_debitor_mapping = []

        [sepa]
        creditors = []
        mandates = []

        [transfer_accounts]
        bank = \"Assets:Bank\"
        cash = \"Assets:Cash\"
        "
        .to_owned();
        let expected = ImporterConfig {
            hledger: HledgerConfig::default(),
            ibans: vec![],
            cards: vec![],
            mapping: vec![],
            creditor_and_debitor_mapping: vec![],
            sepa: SepaConfig {
                creditors: vec![],
                mandates: vec![],
            },
            transfer_accounts: TransferAccounts {
                bank: "Assets:Bank".to_owned(),
                cash: "Assets:Cash".to_owned(),
            },
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);

        let config_str = "ibans = [
            { iban = 'AT123', account = 'Assets:Test1' },
            { iban = 'AT456', account = 'Assets:Test2' },
        ]

        cards = [ { card = \"123XXX456\", account = \"Liabilities:Test\", note = \"Test\" } ]
        mapping = []
        creditor_and_debitor_mapping = []

        [sepa]
        creditors = [
            { creditor_id = \"AT12ZZ0000000\", account = \"Expenses:Test\" }
        ]
        mandates = [
            { mandate_id = \"1234567890\", account = \"Expenses:Test2\" }
        ]

        [transfer_accounts]
        bank = \"Assets:Bank\"
        cash = \"Assets:Cash\"
        "
        .to_owned();
        let expected = ImporterConfig {
            hledger: HledgerConfig::default(),
            mapping: vec![],
            creditor_and_debitor_mapping: vec![],
            transfer_accounts: TransferAccounts {
                bank: "Assets:Bank".to_owned(),
                cash: "Assets:Cash".to_owned(),
            },
            cards: vec![CardMapping {
                card: "123XXX456".to_owned(),
                account: "Liabilities:Test".to_owned(),
                fees_account: None,
                note: Some("Test".to_owned()),
            }],
            sepa: SepaConfig {
                creditors: vec![SepaCreditorMapping {
                    creditor_id: "AT12ZZ0000000".to_owned(),
                    account: "Expenses:Test".to_owned(),
                    note: None,
                }],
                mandates: vec![SepaMandateMapping {
                    mandate_id: "1234567890".to_owned(),
                    account: "Expenses:Test2".to_owned(),
                    note: None,
                }],
            },
            ibans: vec![
                IbanMapping {
                    iban: "AT123".to_owned(),
                    account: "Assets:Test1".to_owned(),
                    fees_account: None,
                    note: None,
                },
                IbanMapping {
                    iban: "AT456".to_owned(),
                    account: "Assets:Test2".to_owned(),
                    fees_account: None,
                    note: None,
                },
            ],
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);

        let config_str = "ibans = []
        cards = []
        mapping = [
          { search = \"Store\", account = \"Expenses:Test\" },
          { search = \"Lab\", account = \"Expenses:Lab\", note = \"Note Test\" },
        ]
        creditor_and_debitor_mapping = [
          { search = \"Special Store\", account = \"Liabilities:AP:Sepcial\", default_pl_account = \"Expenses:Specials\" },
        ]

        [sepa]
        creditors = []
        mandates = []

        [transfer_accounts]
        bank = \"Assets:Bank\"
        cash = \"Assets:Cash\"
        "
        .to_owned();
        let expected = ImporterConfig {
            hledger: HledgerConfig::default(),
            mapping: vec![
                SimpleMapping {
                    search: "Store".to_owned(),
                    account: "Expenses:Test".to_owned(),
                    note: None,
                },
                SimpleMapping {
                    search: "Lab".to_owned(),
                    account: "Expenses:Lab".to_owned(),
                    note: Some("Note Test".to_owned()),
                },
            ],
            creditor_and_debitor_mapping: vec![CreditorDebitorMapping {
                search: "Special Store".to_owned(),
                account: "Liabilities:AP:Sepcial".to_owned(),
                default_pl_account: Some("Expenses:Specials".to_owned()),
            }],
            transfer_accounts: TransferAccounts {
                bank: "Assets:Bank".to_owned(),
                cash: "Assets:Cash".to_owned(),
            },
            cards: vec![],
            sepa: SepaConfig {
                creditors: vec![],
                mandates: vec![],
            },
            ibans: vec![],
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);
    }
}
