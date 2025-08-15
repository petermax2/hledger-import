use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use regex::Regex;
use serde::Deserialize;

use crate::pdftotext;
use crate::{
    HledgerImporter,
    hledger::output::{AmountAndCommodity, Posting, TransactionState},
};
use crate::{config::ImporterConfig, error::*, hledger::output::Transaction};

pub struct FlatexPdfInvoiceImporter {}

impl FlatexPdfInvoiceImporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FlatexPdfInvoiceImporter {
    fn default() -> Self {
        FlatexPdfInvoiceImporter::new()
    }
}

impl HledgerImporter for FlatexPdfInvoiceImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &crate::config::ImporterConfig,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        let text = pdftotext::extract_text_from_pdf(&config.poppler, input_file)?;
        let transaction = self.try_into_hledger(config, &text)?;
        Ok(vec![transaction])
    }

    fn output_title(&self) -> &'static str {
        "flatex import"
    }
}

impl FlatexPdfInvoiceImporter {
    fn try_into_hledger(&self, config: &ImporterConfig, text: &str) -> Result<Transaction> {
        let flatex_conf = match &config.flatex_pdf {
            Some(conf) => conf,
            None => return Err(ImportError::MissingConfig("flatex_pdf".to_owned())),
        };

        let date: NaiveDate =
            FlatexPdfRegexMatcher::new(text, &flatex_conf.date_search, "transaction date")?
                .try_into()?;

        let code = FlatexPdfRegexMatcher::new(text, &flatex_conf.code_search, "transaction code")?
            .first_capture();

        let payee = FlatexPdfRegexMatcher::new(
            text,
            &flatex_conf.payee_search,
            "stock exchange or bank institute",
        )?
        .first_capture()
        .ok_or(ImportError::MissingValue(
            "stock exchange or bank institute".to_owned(),
        ))?;

        let total: AmountAndCommodity =
            FlatexPdfRegexMatcher::new(text, &flatex_conf.total_amount_search, "total amount")?
                .try_into()?;

        // prepare postings
        let mut postings = Vec::new();
        postings.push(Posting {
            account: flatex_conf.settlement_account.clone(),
            amount: Some(total),
            comment: None,
            tags: vec![],
        });

        for posting_rule in &flatex_conf.postings {
            let amount: AmountAndCommodity = FlatexPdfRegexMatcher::new(
                text,
                &posting_rule.search_for,
                &posting_rule.description,
            )?
            .try_into()?;

            let should_post = match &posting_rule.post_if {
                FlatexPostIfConfig::Always => true,
                FlatexPostIfConfig::Positive => amount.amount > bigdecimal::BigDecimal::zero(),
                FlatexPostIfConfig::Negative => amount.amount < bigdecimal::BigDecimal::zero(),
            };

            if !should_post {
                continue;
            }

            postings.push(Posting {
                account: posting_rule.account.clone(),
                amount: Some(amount),
                comment: Some(posting_rule.description.clone()),
                tags: vec![],
            })
        }

        let commodity_amount: BigDecimal = FlatexPdfRegexMatcher::new(
            text,
            &flatex_conf.commodity_amount_search,
            "commodity amount",
        )?
        .try_into()?;

        let mut commodity = None;
        for commodity_rule in &flatex_conf.commodities {
            let matching =
                FlatexPdfRegexMatcher::new(text, &commodity_rule.search_for, "commodity")?
                    .any_match();
            if matching {
                commodity = Some(commodity_rule);
                break;
            }
        }

        if let Some(commodity) = commodity {
            postings.push(Posting {
                account: commodity.asset_account.clone(),
                amount: Some(AmountAndCommodity {
                    amount: commodity_amount.clone(),
                    commodity: commodity.commodity.clone(),
                }),
                comment: None,
                tags: vec![],
            });
            postings.push(Posting {
                account: commodity.conversion_account.clone(),
                amount: None,
                comment: None,
                tags: vec![],
            });
        }

        Ok(Transaction {
            date,
            code,
            payee,
            note: None,
            state: TransactionState::Cleared,
            comment: None,
            tags: vec![],
            postings,
        })
    }
}

struct FlatexPdfRegexMatcher<'a> {
    text: &'a str,
    regex: Regex,
    value_description: &'a str,
}

impl<'a> FlatexPdfRegexMatcher<'a> {
    pub fn new(text: &'a str, regex: &str, value_description: &'a str) -> Result<Self> {
        let regex = Regex::new(regex)?;

        Ok(Self {
            text,
            regex,
            value_description,
        })
    }

    pub fn first_capture(&self) -> Option<String> {
        if let Some(captures) = self.regex.captures(self.text) {
            if let Some(capture) = captures.get(1) {
                return Some(capture.as_str().to_owned());
            }
        }
        None
    }

    pub fn any_match(&self) -> bool {
        self.regex.is_match(self.text)
    }
}

impl TryInto<NaiveDate> for FlatexPdfRegexMatcher<'_> {
    type Error = ImportError;

    fn try_into(self) -> std::prelude::v1::Result<NaiveDate, Self::Error> {
        let value = self
            .first_capture()
            .ok_or(ImportError::MissingValue(self.value_description.to_owned()))?;

        NaiveDate::parse_from_str(&value, "%d.%m.%Y")
            .map_err(|e| ImportError::InputParse(e.to_string()))
    }
}

impl TryInto<AmountAndCommodity> for FlatexPdfRegexMatcher<'_> {
    type Error = ImportError;

    fn try_into(self) -> std::prelude::v1::Result<AmountAndCommodity, Self::Error> {
        let value = self
            .first_capture()
            .ok_or(ImportError::MissingValue(self.value_description.to_owned()))?;

        // split number from commodity
        let mut parts = value.split(' ');
        let number = parts
            .next()
            .ok_or(ImportError::MissingValue(self.value_description.to_owned()))?;
        let number = number.replace('.', "");
        let commodity = parts
            .next()
            .ok_or(ImportError::MissingValue(self.value_description.to_owned()))?;

        // parse number as BigDecimal
        let parts = number.split(',');
        let part_lens: Vec<usize> = parts.into_iter().map(|p| p.len()).collect();
        let decimal_len = if part_lens.len() > 1 {
            part_lens[1]
        } else {
            0_usize
        };

        let number = number.replace(',', "");
        let amount = match BigDecimal::from_str(&number) {
            Ok(b) => b / ((10_u32).pow(decimal_len as u32)),
            Err(e) => return Err(ImportError::InputParse(e.to_string())),
        };

        Ok(AmountAndCommodity {
            amount,
            commodity: commodity.to_owned(),
        })
    }
}

impl TryInto<BigDecimal> for FlatexPdfRegexMatcher<'_> {
    type Error = ImportError;

    fn try_into(self) -> std::prelude::v1::Result<BigDecimal, Self::Error> {
        let value = self
            .first_capture()
            .ok_or(ImportError::MissingValue(self.value_description.to_owned()))?;

        let parts = value.split(',');
        let part_lens: Vec<usize> = parts.into_iter().map(|p| p.len()).collect();
        let decimal_len = if part_lens.len() > 1 {
            part_lens[1]
        } else {
            0_usize
        };

        let number = value.replace(',', "");
        match BigDecimal::from_str(&number) {
            Ok(b) => Ok(b / ((10_u32).pow(decimal_len as u32))),
            Err(e) => Err(ImportError::InputParse(e.to_string())),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexPdfConfig {
    pub settlement_account: String,
    pub total_amount_search: String,
    pub commodity_amount_search: String,
    pub code_search: String,
    pub date_search: String,
    pub payee_search: String,
    #[serde(default)]
    pub commodities: Vec<FlatexCommodityConfig>,
    #[serde(default)]
    pub postings: Vec<FlatexPostingConfig>,
    #[serde(default)]
    pub tags: Vec<FlatexTagConfig>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexCommodityConfig {
    pub search_for: String,
    pub commodity: String,
    pub asset_account: String,
    pub conversion_account: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexPostingConfig {
    pub search_for: String,
    pub account: String,
    pub description: String,
    #[serde(default)]
    pub post_if: FlatexPostIfConfig,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Default)]
pub enum FlatexPostIfConfig {
    #[default]
    Always,
    Positive,
    Negative,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexTagConfig {
    pub search_for: String,
    pub tag: String,
}
