use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use regex::RegexBuilder;
use serde::Deserialize;

use crate::error::Result;
use crate::hledger::output::AmountAndCommodity;
use crate::{
    error::ImportError,
    hledger::output::{Posting, Tag, Transaction, TransactionState},
    HledgerImporter,
};

pub struct RevolutCsvImporter {}

impl RevolutCsvImporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for RevolutCsvImporter {
    fn default() -> Self {
        RevolutCsvImporter::new()
    }
}

impl HledgerImporter for RevolutCsvImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &crate::config::ImporterConfig,
        _known_codes: &std::collections::HashSet<String>,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        let mut transactions = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b',')
            .has_headers(true)
            .double_quote(false)
            .flexible(true)
            .from_path(input_file);
        match &mut reader {
            Ok(reader) => {
                for record in reader.deserialize::<RevolutTransaction>() {
                    match record {
                        Ok(record) => transactions.push(record.into_hledger(config)?),
                        Err(e) => return Err(ImportError::InputParse(e.to_string())),
                    }
                }
            }
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        }
        Ok(transactions)
    }

    fn output_title(&self) -> &'static str {
        "cardcomplete Import"
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RevolutConfig {
    pub account: String,
    pub fee_account: Option<String>,
}

#[derive(Deserialize)]
struct RevolutTransaction {
    #[serde(rename = "Type")]
    pub transaction_type: String,
    // #[serde(rename = "Product")]
    // pub product: String,
    #[serde(rename = "Started Date")]
    pub started_date: String,
    #[serde(rename = "Completed Date")]
    pub completed_date: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "Fee")]
    pub fee: String,
    #[serde(rename = "Currency")]
    pub currency: String,
    #[serde(rename = "State")]
    pub state: String,
    // #[serde(rename = "Balance")]
    // pub balance: String,
}

impl RevolutTransaction {
    pub fn into_hledger(self, config: &crate::config::ImporterConfig) -> Result<Transaction> {
        let state = self.state();
        let tags = self.tags();
        let postings = self.postings(config);

        let date = match NaiveDate::parse_from_str(&self.completed_date[..10], "%Y-%m-%d") {
            Ok(date) => date,
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        };

        Ok(Transaction {
            payee: self.description,
            code: None,
            note: None,
            comment: None,
            date,
            state,
            tags,
            postings: postings?,
        })
    }

    pub fn state(&self) -> TransactionState {
        if self.state.to_uppercase() == "COMPLETED" {
            TransactionState::Cleared
        } else {
            TransactionState::Pending
        }
    }

    pub fn tags(&self) -> Vec<Tag> {
        let valuation_str = self.started_date.clone();
        let type_str = self.transaction_type.clone();

        vec![
            Tag {
                name: "valuation".to_owned(),
                value: Some(valuation_str),
            },
            Tag {
                name: "revolut_type".to_owned(),
                value: Some(type_str),
            },
        ]
    }

    pub fn postings(&self, config: &crate::config::ImporterConfig) -> Result<Vec<Posting>> {
        let revolut_account = match &config.revolut {
            Some(config) => config.account.clone(),
            None => return Err(ImportError::MissingConfig("revolut".to_owned())),
        };
        let revolut_amount = AmountAndCommodity {
            amount: self.amount()?,
            commodity: self.currency.clone(),
        };

        let fee_amount = AmountAndCommodity {
            amount: self.fee()?,
            commodity: self.currency.clone(),
        };

        let other_account = if &self.transaction_type == "TOPUP" {
            Some(config.transfer_accounts.bank.clone())
        } else {
            let mut simple_account: Option<String> = None;
            for rule in &config.mapping {
                let regex = RegexBuilder::new(&rule.search)
                    .case_insensitive(true)
                    .build();
                match regex {
                    Ok(regex) => {
                        if regex.is_match(&self.description) {
                            simple_account = Some(rule.account.clone());
                            break;
                        }
                    }
                    Err(e) => return Err(ImportError::Regex(e.to_string())),
                };
            }
            simple_account
        };

        let mut postings = vec![Posting {
            account: revolut_account,
            amount: Some(revolut_amount),
            comment: None,
            tags: Vec::new(),
        }];

        if fee_amount.amount != BigDecimal::zero() {
            if let Some(config) = &config.revolut {
                if let Some(fee_account) = &config.fee_account {
                    postings.push(Posting {
                        account: fee_account.clone(),
                        amount: Some(fee_amount),
                        comment: Some("fee".to_owned()),
                        tags: Vec::new(),
                    });
                }
            }
        }

        if let Some(other_account) = other_account {
            postings.push(Posting {
                account: other_account,
                amount: None,
                comment: None,
                tags: Vec::new(),
            });
        }

        Ok(postings)
    }

    pub fn amount(&self) -> Result<BigDecimal> {
        RevolutTransaction::amount_str_to_bigdecimal(&self.amount)
    }

    pub fn fee(&self) -> Result<BigDecimal> {
        RevolutTransaction::amount_str_to_bigdecimal(&self.fee)
    }

    fn amount_str_to_bigdecimal(amount_str: &str) -> Result<BigDecimal> {
        let parts = amount_str.split('.');
        let part_lens: Vec<usize> = parts.into_iter().map(|p| p.len()).collect();
        let decimal_len = if part_lens.len() > 1 {
            part_lens[1]
        } else {
            0_usize
        };

        let amount_filtered = amount_str.replace('.', "");

        let big_dec = match BigDecimal::from_str(&amount_filtered) {
            Ok(b) => b / ((10_u32).pow(decimal_len as u32)),
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        };

        Ok(big_dec)
    }
}
