use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::config::ImporterConfigTarget;
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
        "Revolut Import"
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
            Some(ImporterConfigTarget {
                account: config.transfer_accounts.bank.clone(),
                note: None,
            })
        } else {
            config
                .match_mapping(&self.description)?
                .or(config.fallback())
        };

        let mut postings = vec![Posting {
            account: revolut_account.clone(),
            amount: Some(revolut_amount),
            comment: None,
            tags: Vec::new(),
        }];

        if fee_amount.amount != BigDecimal::zero() {
            postings.push(Posting {
                account: revolut_account.clone(),
                amount: Some(AmountAndCommodity {
                    amount: fee_amount.amount.clone() * (-1),
                    commodity: fee_amount.commodity.clone(),
                }),
                comment: Some("fee".to_owned()),
                tags: Vec::new(),
            });

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
                account: other_account.account,
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

#[cfg(test)]
mod tests {
    use bigdecimal::FromPrimitive;

    use crate::config::{
        HledgerConfig, ImporterConfig, SepaConfig, SimpleMapping, TransferAccounts,
    };

    use super::*;

    #[test]
    fn deserialize_csv_examples() {
        let config = test_config();

        let csv = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance
CARD_PAYMENT,Current,2024-05-01 13:05:33,2024-05-01 16:46:56,Patreon,-24.40,0.00,EUR,COMPLETED,100.00
CARD_PAYMENT,Current,2024-05-03 15:04:58,2024-05-04 03:36:34,Apple,-1.99,0.00,EUR,COMPLETED,97.01
TOPUP,Current,2024-05-19 10:02:45,2024-05-22 10:02:45,Payment from John Doe Jr,150.00,0.00,EUR,COMPLETED,247.01
";

        let mut transactions: Vec<Transaction> = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b',')
            .has_headers(true)
            .double_quote(false)
            .flexible(true)
            .from_reader(csv.as_bytes());

        for record in reader.deserialize::<RevolutTransaction>() {
            let record = record.expect("Parsing CSV record failed");
            transactions.push(
                record
                    .into_hledger(&config)
                    .expect("Converting CSV record into hledger output failed"),
            );
        }
        dbg!(&transactions);

        assert_eq!(3, transactions.len());

        let t1 = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
            code: None,
            payee: "Patreon".to_owned(),
            note: None,
            state: TransactionState::Cleared,
            comment: None,
            tags: vec![
                Tag {
                    name: "valuation".to_owned(),
                    value: Some("2024-05-01 13:05:33".to_owned()),
                },
                Tag {
                    name: "revolut_type".to_owned(),
                    value: Some("CARD_PAYMENT".to_owned()),
                },
            ],
            postings: vec![
                Posting {
                    account: "Assets:Revolut".to_owned(),
                    amount: Some(AmountAndCommodity {
                        amount: BigDecimal::from_i64(-2440).unwrap() / 100,
                        commodity: "EUR".to_owned(),
                    }),
                    comment: None,
                    tags: Vec::new(),
                },
                Posting {
                    account: "Expenses:Donation".to_owned(),
                    amount: None,
                    comment: None,
                    tags: Vec::new(),
                },
            ],
        };

        dbg!(&t1);
        assert!(transactions.contains(&t1));

        let t2 = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 5, 4).unwrap(),
            code: None,
            payee: "Apple".to_owned(),
            note: None,
            state: TransactionState::Cleared,
            comment: None,
            tags: vec![
                Tag {
                    name: "valuation".to_owned(),
                    value: Some("2024-05-03 15:04:58".to_owned()),
                },
                Tag {
                    name: "revolut_type".to_owned(),
                    value: Some("CARD_PAYMENT".to_owned()),
                },
            ],
            postings: vec![
                Posting {
                    account: "Assets:Revolut".to_owned(),
                    amount: Some(AmountAndCommodity {
                        amount: BigDecimal::from_i64(-199).unwrap() / 100,
                        commodity: "EUR".to_owned(),
                    }),
                    comment: None,
                    tags: Vec::new(),
                },
                Posting {
                    account: "Expenses:Apples".to_owned(),
                    amount: None,
                    comment: None,
                    tags: Vec::new(),
                },
            ],
        };

        dbg!(&t2);
        assert!(transactions.contains(&t2));

        let t3 = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 5, 22).unwrap(),
            code: None,
            payee: "Payment from John Doe Jr".to_owned(),
            note: None,
            state: TransactionState::Cleared,
            comment: None,
            tags: vec![
                Tag {
                    name: "valuation".to_owned(),
                    value: Some("2024-05-19 10:02:45".to_owned()),
                },
                Tag {
                    name: "revolut_type".to_owned(),
                    value: Some("TOPUP".to_owned()),
                },
            ],
            postings: vec![
                Posting {
                    account: "Assets:Revolut".to_owned(),
                    amount: Some(AmountAndCommodity {
                        amount: BigDecimal::from_i64(150).unwrap(),
                        commodity: "EUR".to_owned(),
                    }),
                    comment: None,
                    tags: Vec::new(),
                },
                Posting {
                    account: "Assets:Reconciliation:Bank".to_owned(),
                    amount: None,
                    comment: None,
                    tags: Vec::new(),
                },
            ],
        };

        dbg!(&t3);
        assert!(transactions.contains(&t3));
    }

    fn test_config() -> ImporterConfig {
        ImporterConfig {
            hledger: HledgerConfig::default(),
            commodity_formatting_rules: None,
            ibans: Vec::new(),
            cards: Vec::new(),
            mapping: vec![
                SimpleMapping {
                    search: "PATREON".to_owned(),
                    account: "Expenses:Donation".to_owned(),
                    note: None,
                },
                SimpleMapping {
                    search: "APPLE".to_owned(),
                    account: "Expenses:Apples".to_owned(),
                    note: None,
                },
            ],
            categories: vec![],
            creditor_and_debitor_mapping: Vec::new(),
            sepa: SepaConfig {
                creditors: Vec::new(),
                mandates: Vec::new(),
            },
            transfer_accounts: TransferAccounts {
                bank: "Assets:Reconciliation:Bank".to_owned(),
                cash: "Assets:Reconciliation:Cash".to_owned(),
            },
            filter: crate::config::WordFilter::default(),
            fallback_account: Some("Equity:Fallback".to_owned()),
            revolut: Some(RevolutConfig {
                account: "Assets:Revolut".to_owned(),
                fee_account: Some("Expenses:Fee".to_owned()),
            }),
            #[cfg(feature = "flatex")]
            flatex_csv: None,
            #[cfg(feature = "flatex")]
            flatex_pdf: None,
        }
    }
}
