#[cfg(feature = "paypal")]
use crate::importers::paypal::PayPalConfig;
#[cfg(feature = "revolut")]
use crate::importers::revolut::RevolutConfig;
#[cfg(feature = "flatex")]
use crate::importers::{flatex_csv::FlatexCsvConfig, flatex_inv::FlatexPdfConfig};

use crate::error::{ImportError, Result};
use homedir::my_home;
use regex::RegexBuilder;
use serde::Deserialize;
use std::{collections::HashSet, str::FromStr};

/// encapsulation of the application configuration
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ImporterConfig {
    #[serde(default)]
    pub hledger: HledgerConfig,
    pub commodity_formatting_rules: Option<Vec<String>>,
    pub deduplication_accounts: Option<HashSet<String>>,
    pub ibans: Vec<IbanMapping>,
    pub cards: Vec<CardMapping>,
    pub mapping: Vec<SimpleMapping>,
    #[serde(default)]
    pub categories: Vec<CategoryMapping>,
    pub creditor_and_debitor_mapping: Vec<CreditorDebitorMapping>,
    pub sepa: SepaConfig,
    pub transfer_accounts: TransferAccounts,
    #[serde(default)]
    pub filter: WordFilter,
    /// a fallback account can be set to balance postings that could not be assigned to any other account
    pub fallback_account: Option<String>,
    #[cfg(feature = "revolut")]
    pub revolut: Option<RevolutConfig>,
    #[cfg(feature = "flatex")]
    pub flatex_csv: Option<FlatexCsvConfig>,
    #[cfg(feature = "flatex")]
    pub flatex_pdf: Option<FlatexPdfConfig>,
    #[cfg(feature = "paypal")]
    pub paypal: Option<PayPalConfig>,
}

impl ImporterConfig {
    pub fn path() -> Result<std::path::PathBuf> {
        let env_path = std::env::var("HLEDGER_IMPORT_CONFIG");
        match env_path {
            Ok(env) => match std::path::PathBuf::from_str(&env) {
                Ok(path) => Ok(path),
                Err(_) => Err(ImportError::ConfigPath),
            },
            Err(_) => match my_home() {
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

    pub fn identify_iban_opt(&self, iban: &Option<String>) -> Option<ImporterConfigTarget> {
        match iban {
            Some(iban) => self.identify_iban(iban),
            None => None,
        }
    }

    pub fn identify_iban(&self, iban: &str) -> Option<ImporterConfigTarget> {
        self.ibans
            .iter()
            .find(|rule| rule.iban == iban)
            .map(|rule| ImporterConfigTarget {
                account: rule.account.clone(),
                note: rule.note.clone(),
            })
    }

    pub fn identify_card_opt(&self, card_number: &Option<String>) -> Option<ImporterConfigTarget> {
        match card_number {
            Some(card_number) => self.identify_card(card_number),
            None => None,
        }
    }

    pub fn identify_card(&self, card_number: &str) -> Option<ImporterConfigTarget> {
        self.cards
            .iter()
            .find(|rule| rule.card == card_number)
            .map(|rule| ImporterConfigTarget {
                account: rule.account.clone(),
                note: rule.note.clone(),
            })
    }

    pub fn match_category(&self, category: &str) -> Option<ImporterConfigTarget> {
        self.categories
            .iter()
            .find(|rule| category.contains(&rule.pattern))
            .map(|rule| ImporterConfigTarget {
                account: rule.account.clone(),
                note: rule.note.clone(),
            })
    }

    pub fn match_sepa_creditor_opt(
        &self,
        sepa_creditor_id: &Option<String>,
    ) -> Option<ImporterConfigTarget> {
        match sepa_creditor_id {
            Some(sepa_creditor_id) => self.match_sepa_creditor(sepa_creditor_id),
            None => None,
        }
    }

    pub fn match_sepa_creditor(&self, sepa_creditor_id: &str) -> Option<ImporterConfigTarget> {
        self.sepa
            .creditors
            .iter()
            .find(|rule| rule.creditor_id == sepa_creditor_id)
            .map(|rule| ImporterConfigTarget {
                account: rule.account.clone(),
                note: rule.note.clone(),
            })
    }

    pub fn match_sepa_mandate_opt(
        &self,
        sepa_mandate_id: &Option<String>,
    ) -> Option<ImporterConfigTarget> {
        match sepa_mandate_id {
            Some(sepa_mandate_id) => self.match_sepa_mandate(sepa_mandate_id),
            None => None,
        }
    }

    pub fn match_sepa_mandate(&self, sepa_mandate_id: &str) -> Option<ImporterConfigTarget> {
        self.sepa
            .mandates
            .iter()
            .find(|rule| rule.mandate_id == sepa_mandate_id)
            .map(|rule| ImporterConfigTarget {
                account: rule.account.clone(),
                note: rule.note.clone(),
            })
    }

    pub fn match_mapping_opt(
        &self,
        field: &Option<String>,
    ) -> Result<Option<ImporterConfigTarget>> {
        match field {
            Some(field) => self.match_mapping(field),
            None => Ok(None),
        }
    }

    pub fn match_mapping(&self, field: &str) -> Result<Option<ImporterConfigTarget>> {
        for rule in &self.mapping {
            if rule.matches(field)? {
                return Ok(Some(ImporterConfigTarget {
                    account: rule.account.clone(),
                    note: rule.note.clone(),
                }));
            }
        }
        Ok(None)
    }

    pub fn fallback(&self) -> Option<ImporterConfigTarget> {
        self.fallback_account
            .as_ref()
            .map(|fallback| ImporterConfigTarget {
                account: fallback.clone(),
                note: None,
            })
    }
}

#[derive(Debug)]
pub struct ImporterConfigTarget {
    pub account: String,
    pub note: Option<String>,
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

impl SimpleMapping {
    pub fn matches(&self, field: &str) -> Result<bool> {
        let regex = RegexBuilder::new(&self.search)
            .case_insensitive(true)
            .build()?;
        Ok(!field.is_empty() && regex.is_match(field))
    }
}

/// Represents a more complex mapping that enables the importer to post to different accounts,
/// depending on the given transaction
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CreditorDebitorMapping {
    pub payee: String,
    pub account: String,
    pub default_pl_account: Option<String>,
    pub days_difference: Option<u32>,
}

/// Define filters to remove or replace certain words from resulting hledger transactions
#[derive(Debug, Deserialize, PartialEq, Eq, Default)]
pub struct WordFilter {
    pub payee: Vec<FilterEntry>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FilterEntry {
    pub pattern: String,
    pub replacement: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CategoryMapping {
    pub pattern: String,
    pub account: String,
    pub note: Option<String>,
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
        fallback_account = \"Equity:Unassigned\"

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
            commodity_formatting_rules: None,
            deduplication_accounts: None,
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
            filter: WordFilter::default(),
            fallback_account: Some("Equity:Unassigned".to_owned()),
            #[cfg(feature = "revolut")]
            revolut: None,
            categories: vec![],
            #[cfg(feature = "flatex")]
            flatex_csv: None,
            #[cfg(feature = "flatex")]
            flatex_pdf: None,
            #[cfg(feature = "paypal")]
            paypal: None,
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);

        let config_str = "ibans = []
        cards = []
        mapping = []
        creditor_and_debitor_mapping = []
        categories = [
          { pattern = \"cat1\", account = \"Expenses:Cat1\" }
        ]

        [sepa]
        creditors = []
        mandates = []

        [filter]
        payee = [
          { pattern = \"foo\", replacement=\"bar\" },
        ]

        [transfer_accounts]
        bank = \"Assets:Bank\"
        cash = \"Assets:Cash\"
        "
        .to_owned();
        let expected = ImporterConfig {
            hledger: HledgerConfig::default(),
            commodity_formatting_rules: None,
            deduplication_accounts: None,
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
            filter: WordFilter {
                payee: vec![FilterEntry {
                    pattern: "foo".to_owned(),
                    replacement: "bar".to_owned(),
                }],
            },
            fallback_account: None,
            #[cfg(feature = "paypal")]
            paypal: None,
            #[cfg(feature = "revolut")]
            revolut: None,
            #[cfg(feature = "flatex")]
            flatex_csv: None,
            #[cfg(feature = "flatex")]
            flatex_pdf: None,
            categories: vec![CategoryMapping {
                pattern: "cat1".to_owned(),
                account: "Expenses:Cat1".to_owned(),
                note: None,
            }],
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

        [[categories]]
        pattern = \"cat1\"
        account = \"Expenses:Cat1\"

        [[categories]]
        pattern = \"cat2\"
        account = \"Expenses:Cat2\"
        note = \"Note\"

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
            commodity_formatting_rules: None,
            deduplication_accounts: None,
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
            filter: WordFilter::default(),
            fallback_account: None,
            #[cfg(feature = "revolut")]
            revolut: None,
            #[cfg(feature = "flatex")]
            flatex_csv: None,
            #[cfg(feature = "flatex")]
            flatex_pdf: None,
            #[cfg(feature = "paypal")]
            paypal: None,
            categories: vec![
                CategoryMapping {
                    pattern: "cat1".to_owned(),
                    account: "Expenses:Cat1".to_owned(),
                    note: None,
                },
                CategoryMapping {
                    pattern: "cat2".to_owned(),
                    account: "Expenses:Cat2".to_owned(),
                    note: Some("Note".to_owned()),
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
          { payee = \"Special Store\", account = \"Liabilities:AP:Sepcial\", default_pl_account = \"Expenses:Specials\", days_difference = 3 },
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
            commodity_formatting_rules: None,
            deduplication_accounts: None,
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
                payee: "Special Store".to_owned(),
                account: "Liabilities:AP:Sepcial".to_owned(),
                default_pl_account: Some("Expenses:Specials".to_owned()),
                days_difference: Some(3),
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
            filter: WordFilter::default(),
            fallback_account: None,
            #[cfg(feature = "revolut")]
            revolut: None,
            #[cfg(feature = "flatex")]
            flatex_csv: None,
            #[cfg(feature = "flatex")]
            flatex_pdf: None,
            #[cfg(feature = "paypal")]
            paypal: None,
            categories: Vec::new(),
        };
        let result = toml::from_str::<ImporterConfig>(&config_str).expect("TOML parsing failed");
        assert_eq!(result, expected);
    }
}
