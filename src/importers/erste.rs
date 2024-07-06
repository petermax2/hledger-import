use std::collections::HashSet;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use chrono::Days;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::config::ImporterConfig;
use crate::config::ImporterConfigTarget;
use crate::error::ImportError;
use crate::error::Result;
use crate::hledger::output::*;
use crate::hledger::query::query_hledger_by_payee_and_account;
use crate::HledgerImporter;

pub struct HledgerErsteJsonImporter {}

impl HledgerErsteJsonImporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for HledgerErsteJsonImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl HledgerImporter for HledgerErsteJsonImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &ImporterConfig,
        known_codes: &HashSet<String>,
    ) -> Result<Vec<Transaction>> {
        match std::fs::read_to_string(input_file) {
            Ok(content) => match serde_json::from_str::<Vec<ErsteTransaction>>(&content) {
                Ok(transactions) => {
                    let result = transactions
                        .into_iter()
                        .filter(|t| !known_codes.contains(&t.reference_number))
                        .map(|t| t.into_hledger(config))
                        .collect::<Result<Vec<_>>>()?;
                    Ok(result)
                }
                Err(e) => Err(ImportError::InputParse(e.to_string())),
            },
            Err(_) => Err(ImportError::InputFileRead(input_file.to_path_buf())),
        }
    }

    fn output_title(&self) -> &'static str {
        "Erste import"
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErsteTransaction {
    pub booking: String,
    pub valuation: String,
    pub partner_name: Option<String>,
    pub reference: Option<String>,
    pub reference_number: String,
    pub receiver_reference: Option<String>,
    pub partner_account: Option<ErstePartnerAccount>,
    // pub partner_reference: Option<String>,
    pub amount: ErsteAmount,
    pub note: Option<String>,
    // pub card_number: Option<String>,
    // pub virtual_card_number: Option<String>,
    // pub virtual_card_device_name: Option<String>,
    pub sepa_mandate_id: Option<String>,
    pub sepa_creditor_id: Option<String>,
    pub owner_account_number: Option<String>,
    // pub owner_account_title: Option<String>,
}

impl ErsteTransaction {
    fn into_hledger(self, config: &ImporterConfig) -> Result<Transaction> {
        let mut postings = Vec::new();
        let mut note = None;
        let date = self.booking_date()?;
        let tags = self.tags();

        let own_target = config
            .identify_iban_opt(&self.owner_account_number)
            .or(config.identify_card("Erste"));

        if let Some(own_target) = own_target {
            note = own_target.note;
            postings.push(Posting {
                account: own_target.account,
                amount: Some(self.amount.clone().try_into()?),
                comment: None,
                tags: Vec::new(),
            });
        }

        let is_bank_transfer = match &self.partner_account {
            Some(partner_account) => config.identify_iban_opt(&partner_account.iban).is_some(),
            None => false,
        };

        if is_bank_transfer {
            postings.push(Posting {
                account: config.transfer_accounts.bank.clone(),
                amount: None,
                comment: None,
                tags: Vec::new(),
            });
        } else {
            let other_target = config
                .match_sepa_mandate_opt(&self.sepa_mandate_id)
                .or(config.match_sepa_creditor_opt(&self.sepa_creditor_id))
                .or(self.match_creditor_debitor_mapping(config)?)
                .or(config.match_mapping_opt(&self.partner_name)?)
                .or(config.match_mapping_opt(&self.reference)?)
                .or(config.fallback());

            if let Some(other_target) = other_target {
                note.clone_from(&other_target.note);
                postings.push(Posting {
                    account: other_target.account.clone(),
                    amount: None,
                    comment: None,
                    tags: Vec::new(),
                });
            }
        }

        let mut payee = self
            .partner_name
            .or(self.reference)
            .unwrap_or("".to_owned());

        config.filter.payee.iter().for_each(|filter| {
            if payee.contains(&filter.pattern) {
                payee = payee.replace(&filter.pattern, &filter.replacement);
            }
        });

        if let Some(trx_note) = &self.note {
            note = Some(trx_note.clone());
        }

        Ok(Transaction {
            date,
            code: Some(self.reference_number),
            state: TransactionState::Cleared,
            comment: None,
            payee,
            note,
            tags,
            postings,
        })
    }

    fn tags(&self) -> Vec<Tag> {
        let mut tags = Vec::new();
        let valuation = &self.valuation;
        if valuation.len() >= 10 {
            tags.push(Tag {
                name: "valuation".to_owned(),
                value: Some(valuation[..10].to_owned()),
            });
        }
        if let Some(reference) = &self.reference {
            if !reference.is_empty() {
                tags.push(Tag {
                    name: "reference".to_owned(),
                    value: Some(reference.clone()),
                });
            }
        }
        if let Some(partner_account) = &self.partner_account {
            if let Some(partner_iban) = &partner_account.iban {
                if !partner_iban.is_empty() {
                    tags.push(Tag {
                        name: "partner_iban".to_owned(),
                        value: Some(partner_iban.clone()),
                    });
                }
            }
        }
        if let Some(receiver_ref) = &self.receiver_reference {
            if !receiver_ref.is_empty() {
                tags.push(Tag {
                    name: "receiverReference".to_owned(),
                    value: Some(receiver_ref.clone()),
                });
            }
        }
        if let Some(sepa_creditor_id) = &self.sepa_creditor_id {
            if !sepa_creditor_id.is_empty() {
                tags.push(Tag {
                    name: "sepaCreditorId".to_owned(),
                    value: Some(sepa_creditor_id.clone()),
                });
            }
        }
        if let Some(sepa_mandate_id) = &self.sepa_mandate_id {
            if !sepa_mandate_id.is_empty() {
                tags.push(Tag {
                    name: "sepaMandateId".to_owned(),
                    value: Some(sepa_mandate_id.clone()),
                })
            }
        }
        tags
    }

    fn booking_date(&self) -> Result<NaiveDate> {
        if self.booking.len() >= 10 {
            match NaiveDate::parse_from_str(&self.booking[..10], "%Y-%m-%d") {
                Ok(date) => Ok(date),
                Err(e) => Err(ImportError::InputParse(e.to_string())),
            }
        } else {
            Err(ImportError::InputParse(format!(
                "invalid booking date \"{}\"",
                &self.booking
            )))
        }
    }

    fn match_creditor_debitor_mapping(
        &self,
        config: &ImporterConfig,
    ) -> Result<Option<ImporterConfigTarget>> {
        match &self.partner_name {
            Some(partner_name) => {
                let search_amount: AmountAndCommodity = self.amount.clone().try_into()?;

                for rule in &config.creditor_and_debitor_mapping {
                    if !partner_name.contains(&rule.payee) {
                        continue;
                    }

                    let begin = match rule.days_difference {
                        Some(delta) => self
                            .booking_date()?
                            .checked_sub_days(Days::new(delta as u64)),
                        None => None,
                    };
                    let end = match rule.days_difference {
                        Some(delta) => self
                            .booking_date()?
                            .checked_add_days(Days::new(delta as u64 + 1)),
                        None => None,
                    };

                    let hledger_transactions = query_hledger_by_payee_and_account(
                        &config.hledger,
                        &rule.payee,
                        &rule.account,
                        begin,
                        end,
                    )?;

                    let matching_cred_or_deb_trx = hledger_transactions.iter().any(|t| {
                        t.tpostings.iter().any(|p| {
                            p.paccount == rule.account
                                && p.pamount
                                    .clone()
                                    .into_iter()
                                    .filter_map(|a| a.try_into().ok())
                                    .any(|a: AmountAndCommodity| a == search_amount)
                        })
                    });

                    if matching_cred_or_deb_trx {
                        return Ok(Some(ImporterConfigTarget {
                            account: rule.account.clone(),
                            note: None,
                        }));
                    } else if let Some(default_pl_account) = &rule.default_pl_account {
                        return Ok(Some(ImporterConfigTarget {
                            account: default_pl_account.clone(),
                            note: None,
                        }));
                    }
                }
                Ok(None)
            }
            None => Ok(None),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErstePartnerAccount {
    pub iban: Option<String>,
    // pub bic: Option<String>,
    // pub number: Option<String>,
    // pub bank_code: Option<String>,
    // pub country_code: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ErsteAmount {
    pub value: i64,
    pub precision: u32,
    pub currency: String,
}

impl TryFrom<ErsteAmount> for AmountAndCommodity {
    type Error = crate::error::ImportError;

    fn try_from(value: ErsteAmount) -> std::result::Result<Self, Self::Error> {
        let amount = BigDecimal::from_i64(value.value);
        match amount {
            Some(amount) => Ok(Self {
                amount: amount / ((10_i64).pow(value.precision)),
                commodity: value.currency,
            }),
            None => Err(ImportError::NumerConversion(value.value.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn deserialize_json_examples() {
        let json_str = "{
  \"transactionId\": null,
  \"containedTransactionId\": null,
  \"booking\": \"2024-06-03T00:00:00.000+0200\",
  \"valuation\": \"2024-06-01T00:00:00.000+0200\",
  \"transactionDateTime\": null,
  \"partnerName\": \"Test Partner\",
  \"partnerAccount\": {
    \"iban\": \"AT472011199999999999\",
    \"bic\": \"\",
    \"number\": \"\",
    \"bankCode\": \"20111\",
    \"countryCode\": \"AT\",
    \"prefix\": null,
    \"secondaryId\": null
  },
  \"partnerAddress\": null,
  \"partnerStructuredAddress\": null,
  \"partnerReference\": null,
  \"partnerOriginator\": null,
  \"amount\": {
    \"value\": -1500,
    \"precision\": 2,
    \"currency\": \"EUR\"
  },
  \"amountSender\": null,
  \"balance\": null,
  \"reference\": \"\",
  \"referenceNumber\": \"123456789000XXX-00XXXXXXXXXX\",
  \"note\": null,
  \"categories\": null,
  \"favorite\": false,
  \"constantSymbol\": null,
  \"variableSymbol\": null,
  \"specificSymbol\": null,
  \"receiverReference\": \"999999999999\",
  \"receiverAddress\": null,
  \"receiverStructuredAddress\": null,
  \"receiverIdentificationReference\": null,
  \"receiverName\": null,
  \"receiverModeReference\": null,
  \"senderReference\": null,
  \"senderAddress\": null,
  \"senderIdentificationReference\": null,
  \"senderModeReference\": null,
  \"senderOriginator\": null,
  \"cardNumber\": null,
  \"cardLocation\": null,
  \"cardType\": null,
  \"cardBrand\": null,
  \"investmentInstrumentName\": null,
  \"bookingTypeTranslation\": null,
  \"e2eReference\": null,
  \"virtualCardNumber\": null,
  \"virtualCardDeviceName\": null,
  \"virtualCardMobilePaymentApplicationName\": null,
  \"sepaMandateId\": \"\",
  \"sepaCreditorId\": \"\",
  \"sepaPurposeType\": null,
  \"sepaScheme\": null,
  \"instructionName\": null,
  \"loanReference\": null,
  \"paymentMethod\": null,
  \"pinEntry\": null,
  \"ownerOriginator\": null,
  \"ownerAccountNumber\": \"AT672011122222222222\",
  \"ownerAccountTitle\": \"John Doe\",
  \"aliasPay\": null,
  \"ultimateCreditor\": null,
  \"ultimateCreditorStructuredAddress\": null,
  \"ultimateDebtor\": null,
  \"ultimateDebtorStructuredAddress\": null,
  \"merchantName\": null,
  \"exchangeRateValue\": null,
  \"foreignExchangeFee\": null,
  \"transactionFee\": null,
  \"statement\": null,
  \"statementInvoice\": null
}
        ";

        let transaction =
            serde_json::from_str::<ErsteTransaction>(json_str).expect("JSON parsing failed");

        assert_eq!(&transaction.partner_name, &Some("Test Partner".to_owned()));
        assert_eq!(
            &transaction.reference_number,
            "123456789000XXX-00XXXXXXXXXX"
        );
        assert_eq!(
            &transaction.owner_account_number,
            &Some("AT672011122222222222".to_owned())
        );
        assert_eq!(
            &transaction.receiver_reference,
            &Some("999999999999".to_owned())
        );
        // assert_eq!(&transaction.partner_reference, &None);

        assert_eq!(
            transaction
                .booking_date()
                .expect("Booking date should be valid but was not parsed correctly"),
            NaiveDate::from_ymd_opt(2024, 6, 3).unwrap()
        );
        assert_eq!(&transaction.valuation[..10], "2024-06-01");

        assert_eq!(
            &transaction
                .partner_account
                .expect("partner account must not be empty if provided")
                .iban,
            &Some("AT472011199999999999".to_owned())
        );
        // assert_eq!(&transaction.partner_account.bic, &Some("".to_owned()));

        assert_eq!(transaction.amount.value, -1500);
        assert_eq!(transaction.amount.precision, 2);
        assert_eq!(&transaction.amount.currency, "EUR");

        let json_str = "{
  \"transactionId\": null,
  \"containedTransactionId\": null,
  \"booking\": \"2024-06-03T00:00:00.000+0200\",
  \"valuation\": \"2024-06-01T00:00:00.000+0200\",
  \"transactionDateTime\": null,
  \"partnerName\": null,
  \"partnerAccount\": {
    \"iban\": \"\",
    \"bic\": \"\",
    \"number\": \"99999999999\",
    \"bankCode\": \"20111\",
    \"countryCode\": \"AT\",
    \"prefix\": null,
    \"secondaryId\": null
  },
  \"partnerAddress\": null,
  \"partnerStructuredAddress\": null,
  \"partnerReference\": null,
  \"partnerOriginator\": null,
  \"amount\": {
    \"value\": -10000,
    \"precision\": 2,
    \"currency\": \"EUR\"
  },
  \"amountSender\": null,
  \"balance\": null,
  \"reference\": \"TEST STORE\",
  \"referenceNumber\": \"123456789000XXX-00YYYYYYYYYY\",
  \"note\": null,
  \"categories\": null,
  \"favorite\": false,
  \"constantSymbol\": null,
  \"variableSymbol\": null,
  \"specificSymbol\": null,
  \"receiverReference\": \"\",
  \"receiverAddress\": null,
  \"receiverStructuredAddress\": null,
  \"receiverIdentificationReference\": null,
  \"receiverName\": null,
  \"receiverModeReference\": null,
  \"senderReference\": null,
  \"senderAddress\": null,
  \"senderIdentificationReference\": null,
  \"senderModeReference\": null,
  \"senderOriginator\": null,
  \"cardNumber\": null,
  \"cardLocation\": null,
  \"cardType\": null,
  \"cardBrand\": null,
  \"investmentInstrumentName\": null,
  \"bookingTypeTranslation\": null,
  \"e2eReference\": null,
  \"virtualCardNumber\": \"\",
  \"virtualCardDeviceName\": \"\",
  \"virtualCardMobilePaymentApplicationName\": \"\",
  \"sepaMandateId\": \"\",
  \"sepaCreditorId\": \"\",
  \"sepaPurposeType\": null,
  \"sepaScheme\": null,
  \"instructionName\": null,
  \"loanReference\": null,
  \"paymentMethod\": null,
  \"pinEntry\": null,
  \"ownerOriginator\": null,
  \"ownerAccountNumber\": null,
  \"ownerAccountTitle\": \"JOHN DOE\",
  \"aliasPay\": null,
  \"ultimateCreditor\": null,
  \"ultimateCreditorStructuredAddress\": null,
  \"ultimateDebtor\": null,
  \"ultimateDebtorStructuredAddress\": null,
  \"merchantName\": null,
  \"exchangeRateValue\": null,
  \"foreignExchangeFee\": null,
  \"transactionFee\": null,
  \"statement\": null,
  \"statementInvoice\": null
}
        ";

        let transaction =
            serde_json::from_str::<ErsteTransaction>(json_str).expect("JSON parsing failed");

        assert_eq!(&transaction.partner_name, &None);
        assert_eq!(
            &transaction.reference_number,
            "123456789000XXX-00YYYYYYYYYY"
        );
        assert_eq!(&transaction.owner_account_number, &None);
        assert_eq!(&transaction.receiver_reference, &Some("".to_owned()));
        // assert_eq!(&transaction.partner_reference, &None);

        assert_eq!(
            transaction.booking_date().unwrap(),
            NaiveDate::from_ymd_opt(2024, 6, 3).unwrap()
        );
        assert_eq!(&transaction.valuation[..10], "2024-06-01");

        assert_eq!(
            &transaction.partner_account.unwrap().iban,
            &Some("".to_owned())
        );
        // assert_eq!(&transaction.partner_account.bic, &Some("".to_owned()));
        // assert_eq!(
        //     &transaction.partner_account.number,
        //     &Some("99999999999".to_owned())
        // );
        // assert_eq!(
        //     &transaction.partner_account.bank_code,
        //     &Some("20111".to_owned())
        // );
        // assert_eq!(
        //     &transaction.partner_account.country_code,
        //     &Some("AT".to_owned())
        // );

        assert_eq!(transaction.amount.value, -10000);
        assert_eq!(transaction.amount.precision, 2);
        assert_eq!(&transaction.amount.currency, "EUR");
    }

    #[test]
    fn convert_minus_one_cent() {
        let source = ErsteAmount {
            value: -1,
            precision: 2,
            currency: "EUR".to_owned(),
        };

        let target = AmountAndCommodity {
            amount: BigDecimal::from_i64(-1).unwrap() / 100,
            commodity: "EUR".to_owned(),
        };

        assert_eq!(target, source.try_into().unwrap());
    }

    #[test]
    fn json_convert_minus_one_cent() {
        let json_str = "{
  \"transactionId\": null,
  \"containedTransactionId\": null,
  \"booking\": \"2024-06-03T00:00:00.000+0200\",
  \"valuation\": \"2024-06-01T00:00:00.000+0200\",
  \"transactionDateTime\": null,
  \"partnerName\": null,
  \"partnerAccount\": {
    \"iban\": \"\",
    \"bic\": \"\",
    \"number\": \"99999999999\",
    \"bankCode\": \"20111\",
    \"countryCode\": \"AT\",
    \"prefix\": null,
    \"secondaryId\": null
  },
  \"partnerAddress\": null,
  \"partnerStructuredAddress\": null,
  \"partnerReference\": null,
  \"partnerOriginator\": null,
  \"amount\": {
    \"value\": -1,
    \"precision\": 2,
    \"currency\": \"EUR\"
  },
  \"amountSender\": null,
  \"balance\": null,
  \"reference\": \"TEST STORE\",
  \"referenceNumber\": \"123456789000XXX-00YYYYYYYYYY\",
  \"note\": null,
  \"categories\": null,
  \"favorite\": false,
  \"constantSymbol\": null,
  \"variableSymbol\": null,
  \"specificSymbol\": null,
  \"receiverReference\": \"\",
  \"receiverAddress\": null,
  \"receiverStructuredAddress\": null,
  \"receiverIdentificationReference\": null,
  \"receiverName\": null,
  \"receiverModeReference\": null,
  \"senderReference\": null,
  \"senderAddress\": null,
  \"senderIdentificationReference\": null,
  \"senderModeReference\": null,
  \"senderOriginator\": null,
  \"cardNumber\": null,
  \"cardLocation\": null,
  \"cardType\": null,
  \"cardBrand\": null,
  \"investmentInstrumentName\": null,
  \"bookingTypeTranslation\": null,
  \"e2eReference\": null,
  \"virtualCardNumber\": \"\",
  \"virtualCardDeviceName\": \"\",
  \"virtualCardMobilePaymentApplicationName\": \"\",
  \"sepaMandateId\": \"\",
  \"sepaCreditorId\": \"\",
  \"sepaPurposeType\": null,
  \"sepaScheme\": null,
  \"instructionName\": null,
  \"loanReference\": null,
  \"paymentMethod\": null,
  \"pinEntry\": null,
  \"ownerOriginator\": null,
  \"ownerAccountNumber\": null,
  \"ownerAccountTitle\": \"JOHN DOE\",
  \"aliasPay\": null,
  \"ultimateCreditor\": null,
  \"ultimateCreditorStructuredAddress\": null,
  \"ultimateDebtor\": null,
  \"ultimateDebtorStructuredAddress\": null,
  \"merchantName\": null,
  \"exchangeRateValue\": null,
  \"foreignExchangeFee\": null,
  \"transactionFee\": null,
  \"statement\": null,
  \"statementInvoice\": null
}
        ";

        let transaction =
            serde_json::from_str::<ErsteTransaction>(json_str).expect("JSON parsing failed");

        let expected = AmountAndCommodity {
            amount: BigDecimal::from_i64(-1).unwrap() / 100,
            commodity: "EUR".to_owned(),
        };

        assert_eq!(expected, transaction.amount.try_into().unwrap());
    }
}
