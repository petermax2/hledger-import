use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::config::ImporterConfig;
use crate::error::*;
use crate::hledger::output::AmountAndCommodity;
use crate::hledger::output::Posting;
use crate::hledger::output::Tag;
use crate::hledger::output::Transaction;
use crate::hledger::output::TransactionState;
use crate::HledgerImporter;

pub struct FlatexCsvImport {}

impl HledgerImporter for FlatexCsvImport {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &crate::config::ImporterConfig,
        known_codes: &std::collections::HashSet<String>,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        let mut transactions = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .has_headers(true)
            .double_quote(false)
            .flexible(true)
            .from_path(input_file);
        match &mut reader {
            Ok(reader) => {
                for record in reader.deserialize::<FlatexTransaction>() {
                    match record {
                        Ok(record) => {
                            let hledger_rec = record.into_hledger(config)?;
                            if !known_codes.contains(&hledger_rec.code.clone().unwrap()) {
                                transactions.push(hledger_rec);
                            }
                        }
                        Err(e) => return Err(ImportError::InputParse(e.to_string())),
                    }
                }
            }
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        }
        Ok(transactions)
    }

    fn output_title(&self) -> &'static str {
        "flatex import"
    }
}

impl FlatexCsvImport {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FlatexCsvImport {
    fn default() -> Self {
        FlatexCsvImport::new()
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexCsvConfig {
    pub account: String,
}

#[derive(Debug, Deserialize)]
struct FlatexTransaction {
    #[serde(rename = "Buchungstag")]
    pub posting_date: String,
    #[serde(rename = "Valuta")]
    pub valuation_date: String,
    #[serde(rename = "Empfänger")]
    pub recipient_name: String,
    #[serde(rename = "Zahlungspfl.")]
    pub recipient_bank_account: String,
    #[serde(rename = "TA.Nr.")]
    pub transaction_nr: String,
    #[serde(rename = "Buchungsinformationen")]
    pub posting_text: String,
    #[serde(rename = "Betrag")]
    pub amount: String,
    #[serde(rename = "")]
    pub currency: String,
}

impl FlatexTransaction {
    pub fn into_hledger(self, config: &ImporterConfig) -> Result<Transaction> {
        let date = self.posting_date()?;
        let tags = self.tags()?;
        let postings = self.postings(config)?;
        let note = if !self.posting_text.is_empty() {
            Some(self.posting_text)
        } else {
            None
        };

        Ok(Transaction {
            date,
            code: Some(self.transaction_nr),
            payee: self.recipient_name,
            note,
            state: TransactionState::Cleared,
            comment: None,
            tags,
            postings,
        })
    }

    pub fn postings(&self, config: &ImporterConfig) -> Result<Vec<Posting>> {
        let mut postings = Vec::new();

        let flatex_config = match &config.flatex_csv {
            Some(config) => config,
            None => return Err(ImportError::MissingConfig("flatex_csv".to_owned())),
        };

        let amount = self.amount()?;

        postings.push(Posting {
            account: flatex_config.account.clone(),
            amount: Some(amount),
            comment: None,
            tags: Vec::new(),
        });

        let bank_transfer = self
            .recipient_bank_account
            .split('/')
            .any(|iban| config.identify_iban(iban).is_some());

        let other_account = if bank_transfer {
            Some(config.transfer_accounts.bank.clone())
        } else {
            config
                .match_mapping(&self.posting_text)?
                .map(|rule| rule.account.clone())
                .or(config.fallback().map(|fallback| fallback.account.clone()))
        };

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

    pub fn tags(&self) -> Result<Vec<Tag>> {
        let valuation = self.valuation_date()?;
        let valuation = valuation.format("%Y-%m-%d").to_string();

        Ok(vec![
            Tag {
                name: "valuation".to_owned(),
                value: Some(valuation),
            },
            Tag {
                name: "partner_iban".to_owned(),
                value: Some(self.recipient_bank_account.clone()),
            },
        ])
    }

    pub fn amount(&self) -> Result<AmountAndCommodity> {
        let amount = self.amount.replace('.', "");
        let part_lengths: Vec<usize> = amount.split(',').map(|p| p.len()).collect();
        let decimals = if part_lengths.len() > 1 {
            part_lengths[1]
        } else {
            0_usize
        };

        let amount = match BigDecimal::from_str(&amount.replace(',', "")) {
            Ok(big_dec) => big_dec / ((10_u32).pow(decimals as u32)),
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        };

        Ok(AmountAndCommodity {
            amount,
            commodity: self.currency.clone(),
        })
    }

    pub fn posting_date(&self) -> Result<NaiveDate> {
        FlatexTransaction::parse_date(&self.posting_date)
    }

    pub fn valuation_date(&self) -> Result<NaiveDate> {
        FlatexTransaction::parse_date(&self.valuation_date)
    }

    fn parse_date(date: &str) -> Result<NaiveDate> {
        match NaiveDate::parse_from_str(date, "%d.%m.%Y") {
            Ok(date) => Ok(date),
            Err(e) => Err(ImportError::InputParse(e.to_string())),
        }
    }
}
