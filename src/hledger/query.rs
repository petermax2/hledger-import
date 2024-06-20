use std::process::Command;

use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::{config::HledgerConfig, error::*};

use super::output::AmountAndCommodity;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct HledgerJsonTransaction {
    pub tcode: String,
    pub tdate: NaiveDate,
    pub tdate2: Option<NaiveDate>,
    pub tcomment: Option<String>,
    pub tdescription: Option<String>,
    pub tpostings: Vec<HledgerJsonPosting>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct HledgerJsonPosting {
    pub paccount: String,
    pub pcomment: Option<String>,
    pub pamount: Vec<HledgerJsonAmount>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct HledgerJsonAmount {
    pub acommodity: String,
    pub aquantity: HledgerJsonQuantity,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HledgerJsonQuantity {
    pub decimal_mantissa: i64,
    pub decimal_places: u32,
}

impl TryFrom<HledgerJsonQuantity> for BigDecimal {
    type Error = crate::error::ImportError;

    fn try_from(value: HledgerJsonQuantity) -> std::result::Result<Self, Self::Error> {
        match BigDecimal::from_i64(value.decimal_mantissa) {
            Some(d) => Ok(d / (10_i64).pow(value.decimal_places)),
            None => Err(ImportError::NumerConversion(format!(
                "{}",
                value.decimal_mantissa
            ))),
        }
    }
}

impl TryFrom<HledgerJsonAmount> for AmountAndCommodity {
    type Error = crate::error::ImportError;

    fn try_from(value: HledgerJsonAmount) -> std::result::Result<Self, Self::Error> {
        let amount = value.aquantity.try_into()?;
        Ok(AmountAndCommodity {
            amount,
            commodity: value.acommodity.clone(),
        })
    }
}

pub fn query_hledger_by_payee_and_account(
    config: &HledgerConfig,
    payee: &str,
    account: &str,
    begin: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<Vec<HledgerJsonTransaction>> {
    let output = if begin.is_some() && end.is_some() {
        Command::new(&config.path)
            .arg("print")
            .arg("-O")
            .arg("json")
            .arg(format!("payee:{}", payee))
            .arg("-b")
            .arg(begin.unwrap().format("%Y-%m-%d").to_string())
            .arg("-e")
            .arg(end.unwrap().format("%Y-%m-%d").to_string())
            .arg(account)
            .output()
    } else if let Some(begin) = begin {
        Command::new(&config.path)
            .arg("print")
            .arg("-O")
            .arg("json")
            .arg(format!("payee:{}", payee))
            .arg("-b")
            .arg(begin.format("%Y-%m-%d").to_string())
            .arg(account)
            .output()
    } else if let Some(end) = end {
        Command::new(&config.path)
            .arg("print")
            .arg("-O")
            .arg("json")
            .arg(format!("payee:{}", payee))
            .arg("-e")
            .arg(end.format("%Y-%m-%d").to_string())
            .arg(account)
            .output()
    } else {
        Command::new(&config.path)
            .arg("print")
            .arg("-O")
            .arg("json")
            .arg(format!("payee:{}", payee))
            .arg(account)
            .output()
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => return Err(ImportError::HledgerExection(e)),
    };

    let json_str = match std::str::from_utf8(&output.stdout) {
        Ok(c) => c,
        Err(e) => return Err(ImportError::StringConversion(e)),
    };

    match serde_json::from_str(json_str) {
        Ok(result) => Ok(result),
        Err(e) => Err(ImportError::Query(e.to_string())),
    }
}
