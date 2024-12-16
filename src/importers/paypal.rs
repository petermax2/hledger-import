use std::{collections::HashSet, str::FromStr};

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use serde::Deserialize;

use crate::{
    error::*,
    hledger::output::{Tag, Transaction},
};
use crate::{
    hledger::output::{AmountAndCommodity, Posting, TransactionState},
    HledgerImporter,
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
        _known_codes: &std::collections::HashSet<String>,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        let paypal_config = match &config.paypal {
            Some(conf) => conf,
            None => return Err(ImportError::MissingConfig("paypal".to_string())),
        };

        let exclude_types = if let Some(exclude_types) = &paypal_config.exclude_types {
            exclude_types.iter().collect()
        } else {
            HashSet::new()
        };

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

            if exclude_types.contains(&record.transaction_type) {
                continue;
            }

            let transaction = ConfiguredPaypalTransaction {
                config: paypal_config,
                transaction: &record,
            };
            let transaction: Transaction = transaction.try_into()?;
            transactions.push(transaction);
        }

        Ok(transactions)
    }

    fn output_title(&self) -> &'static str {
        "PayPal import"
    }
}

#[derive(Debug, Deserialize)]
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
    pub transaction: &'a PayPalTransaction,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct PayPalConfig {
    pub fees_account: String,
    pub asset_account: String,
    pub clearing_account: String,
    pub empty_payee: String,
    pub exclude_types: Option<Vec<String>>,
}

impl TryInto<Transaction> for ConfiguredPaypalTransaction<'_> {
    type Error = ImportError;

    fn try_into(self) -> std::result::Result<Transaction, Self::Error> {
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
            account: self.config.clearing_account.clone(),
            amount: None,
            comment: None,
            tags: Vec::new(),
        });

        let t = Transaction {
            date,
            postings,
            payee,
            code: None,
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
