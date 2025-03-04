use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use fast_xml::DeError;
use fast_xml::de::from_reader;
use serde::Deserialize;

use crate::HledgerImporter;
use crate::config::ImporterConfig;
use crate::error::*;
use crate::hledger::output::{AmountAndCommodity, Posting, Tag, Transaction, TransactionState};

pub struct CardcompleteXmlImporter {}

impl CardcompleteXmlImporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for CardcompleteXmlImporter {
    fn default() -> Self {
        CardcompleteXmlImporter::new()
    }
}

impl HledgerImporter for CardcompleteXmlImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &crate::config::ImporterConfig,
        _known_codes: &std::collections::HashSet<String>,
    ) -> Result<Vec<Transaction>> {
        let file = match std::fs::File::open(input_file) {
            Ok(file) => file,
            Err(_) => return Err(ImportError::InputFileRead(input_file.to_owned())),
        };

        let reader = std::io::BufReader::new(file);
        let read_result: std::result::Result<CCDocument, DeError> = from_reader(reader);
        match read_result {
            Ok(doc) => {
                let mut result = doc
                    .transactions
                    .into_iter()
                    .map(|t| t.into_hledger(config))
                    .collect::<Result<Vec<_>>>()?;
                result.sort_by(|a, b| a.date.partial_cmp(&b.date).unwrap());
                Ok(result)
            }
            Err(e) => Err(ImportError::InputParse(e.to_string())),
        }
    }

    fn output_title(&self) -> &'static str {
        "cardcomplete import"
    }
}

/// XML root node in Cardcomplete XML export
#[derive(Debug, Deserialize)]
struct CCDocument {
    #[serde(rename = "TRANSACTION")]
    pub transactions: Vec<CCTransaction>,
}

/// XML representation of Cardcomplete transaction export
#[derive(Debug, Deserialize, Default)]
struct CCTransaction {
    #[serde(rename = "HAENLDERNAME-MERCHANT_NAME")]
    pub merchant_name: String,

    #[serde(rename = "BETRAG-AMOUNT")]
    pub amount: String,

    #[serde(rename = "WAEHRUNG-CURRENCY")]
    pub currency: String,

    #[serde(rename = "DATUM-DATE")]
    pub date: String,

    #[serde(rename = "ZEIT-TIME")]
    pub time: String,

    #[serde(rename = "BRANCHE-CATEGORY")]
    pub category: String,

    #[serde(rename = "STATUS-STATUS")]
    pub state: String,

    #[serde(rename = "BUCHUNGSDATUM-POSTING_DATE")]
    pub posting_date: String,

    #[serde(rename = "ORT-PLACE")]
    pub place: Option<String>,

    #[serde(rename = "KARTENNUMMER-CARD_NUMBER")]
    pub card_number: Option<String>,
}

impl CCTransaction {
    pub fn into_hledger(self, config: &ImporterConfig) -> Result<Transaction> {
        let mut note = None;
        let mut postings = Vec::new();

        let posting_date = self.posting_date()?;
        let tags = self.tags()?;
        let state = self.state();

        let own_target = config.identify_card_opt(&self.card_number);
        if let Some(own_target) = own_target {
            note.clone_from(&own_target.note);
            postings.push(Posting {
                account: own_target.account,
                amount: Some(self.amount()?),
                comment: None,
                tags: Vec::new(),
            });
        }

        let other_target = config
            .match_mapping(&self.merchant_name)?
            .or(config.match_category(&self.category))
            .or(config.fallback());
        if let Some(other_target) = other_target {
            note.clone_from(&other_target.note);
            postings.push(Posting {
                account: other_target.account,
                amount: None,
                comment: None,
                tags: Vec::new(),
            });
        }

        Ok(Transaction {
            date: posting_date,
            code: None,
            payee: self.merchant_name,
            note,
            state,
            comment: None,
            tags,
            postings,
        })
    }

    pub fn tags(&self) -> Result<Vec<Tag>> {
        let mut tags = Vec::new();

        let date = self.date()?;
        tags.push(Tag {
            name: "valuation".to_owned(),
            value: Some(date.format("%Y-%m-%d").to_string()),
        });

        if !self.category.is_empty() {
            tags.push(Tag {
                name: "category".to_owned(),
                value: Some(self.category.clone()),
            });
        }

        if let Some(place) = &self.place {
            if !place.is_empty() {
                tags.push(Tag {
                    name: "location".to_owned(),
                    value: Some(place.clone()),
                });
            }
        }

        if !self.time.is_empty() {
            tags.push(Tag {
                name: "time".to_owned(),
                value: Some(self.time.clone()),
            });
        }

        Ok(tags)
    }

    pub fn amount(&self) -> Result<AmountAndCommodity> {
        let parts = self.amount.split(',');
        let parts_lengths: Vec<usize> = parts.into_iter().map(|p| p.len()).collect();
        let decimal_len = if parts_lengths.len() > 1 {
            parts_lengths[1]
        } else {
            0_usize
        };

        let amount_filtered = self.amount.replace(',', "");

        let big_dec = match BigDecimal::from_str(&amount_filtered) {
            Ok(b) => b / ((10_u32).pow(decimal_len as u32)),
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        };

        Ok(AmountAndCommodity {
            amount: big_dec,
            commodity: self.currency.clone(),
        })
    }

    pub fn state(&self) -> TransactionState {
        if &self.state.to_lowercase() == "verbucht" {
            TransactionState::Cleared
        } else {
            TransactionState::Pending
        }
    }

    pub fn date(&self) -> Result<NaiveDate> {
        CCTransaction::parse_date(&self.date)
    }

    pub fn posting_date(&self) -> Result<NaiveDate> {
        CCTransaction::parse_date(&self.posting_date)
    }

    fn parse_date(val: &str) -> Result<NaiveDate> {
        match NaiveDate::parse_from_str(val, "%d.%m.%Y") {
            Ok(date) => Ok(date),
            Err(e) => Err(ImportError::InputParse(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use bigdecimal::FromPrimitive;

    use super::*;

    #[test]
    fn convert_date() {
        let mut t = CCTransaction::default();
        t.date = "25.12.2023".to_owned();

        let expected = NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        let result = t.date().expect("Date parsing failed");

        assert_eq!(result, expected);
    }

    #[test]
    fn convert_posting_date() {
        let mut t = CCTransaction::default();
        t.posting_date = "01.02.2020".to_owned();

        let expected = NaiveDate::from_ymd_opt(2020, 2, 1).unwrap();
        let result = t.posting_date().expect("Date parsing failed");

        assert_eq!(result, expected);
    }

    #[test]
    fn transaction_state() {
        let mut t = CCTransaction::default();
        t.state = "Verbucht".to_owned();

        assert_eq!(TransactionState::Cleared, t.state());

        t = CCTransaction::default();
        t.state = "".to_owned();

        assert_eq!(TransactionState::Pending, t.state());
    }

    #[test]
    fn amount_and_commodity() {
        let mut t = CCTransaction::default();
        t.amount = "-3,70".to_owned();
        t.currency = "EUR".to_owned();

        let expected = AmountAndCommodity {
            amount: BigDecimal::from_i32(-370).unwrap() / 100,
            commodity: "EUR".to_owned(),
        };

        assert_eq!(t.amount().unwrap(), expected);

        t = CCTransaction::default();
        t.amount = "350".to_owned();
        t.currency = "USD".to_owned();

        let expected = AmountAndCommodity {
            amount: BigDecimal::from_i32(350).unwrap(),
            commodity: "USD".to_owned(),
        };

        assert_eq!(t.amount().unwrap(), expected);

        t = CCTransaction::default();
        t.amount = "fail".to_owned();

        assert!(t.amount().is_err());
    }
}
