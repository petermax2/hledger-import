use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use regex::Regex;
use serde::Deserialize;

use crate::{
    HledgerImporter,
    hasher::transaction_hash,
    hledger::output::{AmountAndCommodity, Posting, TransactionState},
};
use crate::{
    error::*,
    hledger::output::{Tag, Transaction},
};

pub struct PaypalPdfImporter {}

impl PaypalPdfImporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for PaypalPdfImporter {
    fn default() -> Self {
        PaypalPdfImporter::new()
    }
}

impl HledgerImporter for PaypalPdfImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &crate::config::ImporterConfig,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        // prepare import configuration
        let paypal_config = match &config.paypal {
            Some(conf) => conf,
            None => return Err(ImportError::MissingConfig("paypal".to_string())),
        };

        // convert the configured rules to regex matchers
        let mut regex_errors = vec![];

        let rules: Vec<PayPalRegexRuleMatcher> = paypal_config
            .rules
            .iter()
            .map(PayPalRegexRuleMatcher::new)
            .filter_map(|r| r.map_err(|e| regex_errors.push(e)).ok())
            .collect();

        if let Some(error) = regex_errors.into_iter().next() {
            return Err(error);
        }

        // read in and parse the paypal transactions
        let mut transactions = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(true)
            .double_quote(true)
            .flexible(true)
            .from_path(input_file)
            .map_err(|e| ImportError::InputParse(e.to_string()))?;

        for record in reader.deserialize::<PayPalTransaction>() {
            let record = record.map_err(|e| ImportError::InputParse(e.to_string()))?;

            for rule in &rules {
                if rule.matches(&record) {
                    let ignore = rule.rule.ignore.unwrap_or(false);
                    if !ignore {
                        let transaction = ConfiguredPaypalTransaction {
                            config: paypal_config,
                            transaction: &record,
                            rule: rule.rule,
                        };
                        let transaction: Transaction = transaction.try_into()?;
                        transactions.push(transaction);
                    }
                    break;
                }
            }
        }

        Ok(transactions)
    }

    fn output_title(&self) -> &'static str {
        "PayPal import"
    }
}

#[derive(Debug, Deserialize, Hash)]
struct PayPalTransaction {
    #[serde(rename = "Datum")]
    pub posting_date: String,
    #[serde[rename = "Uhrzeit"]]
    pub posting_time: String,
    #[serde[rename = "Zeitzone"]]
    pub timezone: String,
    #[serde[rename = "Name"]]
    pub name: String,
    #[serde[rename = "Typ"]]
    pub transaction_type: String,
    #[serde[rename = "Status"]]
    pub status: String,
    #[serde[rename = "Währung"]]
    pub currency: String,
    #[serde[rename = "Brutto"]]
    pub gross_amount: String,
    #[serde[rename = "Gebühr"]]
    pub fee: String,
    #[serde[rename = "Netto"]]
    pub net_amount: String,
}

struct ConfiguredPaypalTransaction<'a> {
    pub config: &'a PayPalConfig,
    pub rule: &'a PayPalMatchingRule,
    pub transaction: &'a PayPalTransaction,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct PayPalConfig {
    pub asset_account: String,
    pub fees_account: String,
    pub empty_payee: String,
    pub rules: Vec<PayPalMatchingRule>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct PayPalMatchingRule {
    pub name: Option<String>,
    #[serde[rename = "type"]]
    pub transaction_type: Option<String>,
    pub ignore: Option<bool>,
    #[serde[rename = "account"]]
    pub offset_account: Option<String>,
}

struct PayPalRegexRuleMatcher<'a> {
    pub name: Option<Regex>,
    pub transaction_type: Option<Regex>,
    pub rule: &'a PayPalMatchingRule,
}

impl<'a> PayPalRegexRuleMatcher<'a> {
    pub fn new(rule: &'a PayPalMatchingRule) -> Result<Self> {
        let name = match &rule.name {
            Some(n) => Some(Regex::new(n).map_err(ImportError::Regex)?),
            None => None,
        };
        let transaction_type = match &rule.transaction_type {
            Some(t) => Some(Regex::new(t).map_err(ImportError::Regex)?),
            None => None,
        };
        Ok(Self {
            name,
            transaction_type,
            rule,
        })
    }

    pub fn matches(&self, transaction: &PayPalTransaction) -> bool {
        if let Some(name) = &self.name {
            if !name.is_match(transaction.name.trim()) {
                return false;
            }
        }
        if let Some(transaction_type) = &self.transaction_type {
            if !transaction_type.is_match(transaction.transaction_type.trim()) {
                return false;
            }
        }
        true
    }
}

impl TryInto<Transaction> for ConfiguredPaypalTransaction<'_> {
    type Error = ImportError;

    fn try_into(self) -> std::result::Result<Transaction, Self::Error> {
        let code = transaction_hash("PAYPAL", &self.transaction);

        let date = NaiveDate::parse_from_str(&self.transaction.posting_date, "%d.%m.%Y")
            .map_err(|e| ImportError::InputParse(e.to_string()))?;

        let payee = if !self.transaction.name.trim().is_empty() {
            self.transaction.name.trim().to_string()
        } else {
            self.config.empty_payee.to_string()
        };

        let gross_amount =
            BigDecimal::from_str(&self.transaction.gross_amount.trim().replace(",", "."))
                .map_err(|e| ImportError::InputParse(e.to_string()))?;

        let gross_amount = AmountAndCommodity {
            amount: gross_amount,
            commodity: self.transaction.currency.clone(),
        };

        let mut postings = vec![Posting {
            account: self.config.asset_account.clone(),
            amount: Some(gross_amount),
            comment: None,
            tags: Vec::new(),
        }];

        let fee_amount = BigDecimal::from_str(&self.transaction.fee.trim().replace(",", "."))
            .map_err(|e| ImportError::InputParse(e.to_string()))?;

        if !fee_amount.is_zero() {
            let fee_amount = AmountAndCommodity {
                amount: fee_amount,
                commodity: self.transaction.currency.clone(),
            };
            postings.push(Posting {
                account: self.config.fees_account.clone(),
                amount: Some(fee_amount),
                comment: Some("transaction fee".to_string()),
                tags: Vec::new(),
            });
        }

        postings.push(Posting {
            account: self.rule.offset_account.clone().unwrap_or("".to_string()),
            amount: None,
            comment: None,
            tags: Vec::new(),
        });

        let t = Transaction {
            date,
            postings,
            payee,
            code: Some(code),
            comment: None,
            state: TransactionState::Cleared,
            note: Some(self.transaction.transaction_type.clone()),
            tags: vec![
                Tag {
                    name: "time".to_string(),
                    value: Some(self.transaction.posting_time.clone()),
                },
                Tag {
                    name: "timezone".to_string(),
                    value: Some(self.transaction.timezone.clone()),
                },
                Tag {
                    name: "status".to_string(),
                    value: Some(self.transaction.status.clone()),
                },
                Tag {
                    name: "net_amount".to_string(),
                    value: Some(self.transaction.net_amount.clone()),
                },
            ],
        };
        Ok(t)
    }
}
