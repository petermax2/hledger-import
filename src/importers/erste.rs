use std::collections::HashSet;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use chrono::DateTime;
use regex::RegexBuilder;
use serde::Deserialize;

use crate::config::CardMapping;
use crate::config::IbanMapping;
use crate::config::ImporterConfig;
use crate::config::SepaCreditorMapping;
use crate::config::SepaMandateMapping;
use crate::config::SimpleMapping;
use crate::error::ImportError;
use crate::error::Result;
use crate::hledger::*;
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
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    Ok(result)
                }
                Err(e) => Err(ImportError::InputParse(e.to_string())),
            },
            Err(_) => Err(ImportError::InputFileRead(input_file.to_path_buf())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErsteTransaction {
    pub booking: DateTime<chrono::Local>,
    pub valuation: DateTime<chrono::Local>,
    pub partner_name: Option<String>,
    pub reference: Option<String>,
    pub reference_number: String,
    pub receiver_reference: Option<String>,
    pub partner_account: ErstePartnerAccount,
    pub partner_reference: Option<String>,
    pub amount: ErsteAmount,
    pub note: Option<String>,
    pub card_number: Option<String>,
    pub virtual_card_number: Option<String>,
    pub virtual_card_device_name: Option<String>,
    pub sepa_mandate_id: Option<String>,
    pub sepa_creditor_id: Option<String>,
    pub owner_account_number: Option<String>,
    pub owner_account_title: Option<String>,
}

impl ErsteTransaction {
    fn into_hledger(self, config: &ImporterConfig) -> Result<Vec<Transaction>> {
        let matching_config = MatchingConfigItems::match_config(&self, config)?;

        let tags = self.derive_tags();
        let postings = self.derive_postings(&matching_config, config)?;
        let note = self
            .note
            .or(matching_config.sepa_creditor.and_then(|c| c.note.clone()))
            .or(matching_config.sepa_mandate.and_then(|m| m.note.clone()))
            .or(matching_config.simple_mapping.and_then(|s| s.note.clone()));

        let mut payee = self
            .partner_name
            .or(self.reference)
            .unwrap_or("".to_owned());

        config.filter.payee.iter().for_each(|filter| {
            if payee.contains(&filter.pattern) {
                payee = payee.replace(&filter.pattern, &filter.replacement);
            }
        });

        Ok(vec![Transaction {
            date: self.booking.date_naive(),
            code: Some(self.reference_number),
            state: TransactionState::Cleared,
            comment: None,
            payee,
            note,
            tags,
            postings,
        }])
    }

    fn derive_tags(&self) -> Vec<Tag> {
        let mut tags = Vec::new();
        tags.push(Tag {
            name: "valuation".to_owned(),
            value: Some(self.valuation.date_naive().format("%Y-%m-%d").to_string()),
        });
        if let Some(reference) = &self.reference {
            if !reference.is_empty() {
                tags.push(Tag {
                    name: "reference".to_owned(),
                    value: Some(reference.clone()),
                });
            }
        }
        if let Some(partner_iban) = &self.partner_account.iban {
            if !partner_iban.is_empty() {
                tags.push(Tag {
                    name: "partner_iban".to_owned(),
                    value: Some(partner_iban.clone()),
                });
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

    fn derive_postings(
        &self,
        config_items: &MatchingConfigItems,
        config: &ImporterConfig,
    ) -> Result<Vec<Posting>> {
        let mut result = Vec::new();

        // posting on main bank account
        let own_account = config_items
            .iban
            .map(|iban| iban.account.clone())
            .or_else(|| config_items.card.map(|card| card.account.clone()));

        if let Some(own_account) = own_account {
            result.push(Posting {
                account: own_account,
                amount: Some(AmountAndCommodity {
                    amount: self.get_amount()?,
                    commodity: self.amount.currency.clone(),
                }),
                comment: None,
                tags: Vec::new(),
            });
        }

        // postings agains another bank account owned by the person results in a bank transfer posting
        if config_items.posting_against_own_iban {
            result.push(Posting {
                account: config.transfer_accounts.bank.clone(),
                amount: None,
                comment: None,
                tags: Vec::new(),
            });
            return Ok(result);
        }

        // posting on P/L account or transfer account
        let other_account = config_items
            .sepa_creditor
            .map(|creditor| creditor.account.clone())
            .or_else(|| {
                config_items
                    .sepa_mandate
                    .map(|mandate| mandate.account.clone())
            })
            .or_else(|| {
                config_items
                    .simple_mapping
                    .map(|mapping| mapping.account.clone())
            });
        if let Some(other_account) = other_account {
            result.push(Posting {
                account: other_account,
                amount: None,
                comment: None,
                tags: Vec::new(),
            });
        }

        Ok(result)
    }

    fn get_amount(&self) -> Result<BigDecimal> {
        let amount = BigDecimal::from_i64(self.amount.value);
        match amount {
            Some(amount) => Ok(amount / ((10_i64).pow(self.amount.precision))),
            None => Err(ImportError::NumerConversion(self.amount.value.to_string())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErstePartnerAccount {
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub number: Option<String>,
    pub bank_code: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErsteAmount {
    pub value: i64,
    pub precision: u32,
    pub currency: String,
}

struct MatchingConfigItems<'a> {
    pub sepa_creditor: Option<&'a SepaCreditorMapping>,
    pub sepa_mandate: Option<&'a SepaMandateMapping>,
    pub iban: Option<&'a IbanMapping>,
    pub card: Option<&'a CardMapping>,
    pub simple_mapping: Option<&'a SimpleMapping>,

    /// this flag is set to true, if the partner IBAN is found in the configuration
    pub posting_against_own_iban: bool,
}

impl<'a> MatchingConfigItems<'a> {
    pub fn match_config(
        transaction: &ErsteTransaction,
        config: &'a ImporterConfig,
    ) -> Result<Self> {
        let mut iban = None;
        if let Some(own_account_nr) = &transaction.owner_account_number {
            if !own_account_nr.is_empty() {
                // bank account (identified by its IBAN)
                let iban_mapping = config.ibans.iter().find(|i| &i.iban == own_account_nr);
                if let Some(iban_mapping) = iban_mapping {
                    iban = Some(iban_mapping);
                }
            }
        }

        let mut card = None;
        let card_mapping = config.cards.iter().find(|c| c.card == "Erste");
        if let Some(card_mapping) = card_mapping {
            card = Some(card_mapping);
        }

        let mut sepa_creditor = None;
        if let Some(creditor_id) = &transaction.sepa_creditor_id {
            if !creditor_id.is_empty() {
                let sepa_creditor_mapping = config
                    .sepa
                    .creditors
                    .iter()
                    .find(|item| item.creditor_id == *creditor_id);
                if let Some(sepa_creditor_mapping) = sepa_creditor_mapping {
                    sepa_creditor = Some(sepa_creditor_mapping);
                }
            }
        }

        let mut sepa_mandate = None;
        if let Some(mandate_id) = &transaction.sepa_mandate_id {
            if !mandate_id.is_empty() {
                let sepa_mandate_mapping = config
                    .sepa
                    .mandates
                    .iter()
                    .find(|item| item.mandate_id == *mandate_id);
                if let Some(sepa_mandate_mapping) = sepa_mandate_mapping {
                    sepa_mandate = Some(sepa_mandate_mapping);
                }
            }
        }

        let mut simple_mapping = None;
        for rule in &config.mapping {
            let regex = RegexBuilder::new(&rule.search)
                .case_insensitive(true)
                .build();
            match regex {
                Ok(regex) => {
                    if let Some(partner_name) = &transaction.partner_name {
                        if !partner_name.is_empty() && regex.is_match(partner_name) {
                            simple_mapping = Some(rule);
                            break;
                        }
                    }

                    if let Some(reference) = &transaction.reference {
                        if !reference.is_empty() && regex.is_match(reference) {
                            simple_mapping = Some(rule);
                            break;
                        }
                    }
                }
                Err(e) => return Err(ImportError::Regex(e.to_string())),
            };
        }

        let posting_against_own_iban = match &transaction.partner_account.iban {
            Some(iban) => config
                .ibans
                .iter()
                .any(|iban_mapping| iban_mapping.iban == *iban),
            None => false,
        };

        Ok(Self {
            iban,
            card,
            sepa_creditor,
            sepa_mandate,
            simple_mapping,
            posting_against_own_iban,
        })
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
        assert_eq!(&transaction.partner_reference, &None);

        assert_eq!(
            transaction.booking.date_naive(),
            NaiveDate::from_ymd_opt(2024, 6, 3).unwrap()
        );
        assert_eq!(
            transaction.valuation.date_naive(),
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()
        );

        assert_eq!(
            &transaction.partner_account.iban,
            &Some("AT472011199999999999".to_owned())
        );
        assert_eq!(&transaction.partner_account.bic, &Some("".to_owned()));

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
        assert_eq!(&transaction.partner_reference, &None);

        assert_eq!(
            transaction.booking.date_naive(),
            NaiveDate::from_ymd_opt(2024, 6, 3).unwrap()
        );
        assert_eq!(
            transaction.valuation.date_naive(),
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()
        );

        assert_eq!(&transaction.partner_account.iban, &Some("".to_owned()));
        assert_eq!(&transaction.partner_account.bic, &Some("".to_owned()));
        assert_eq!(
            &transaction.partner_account.number,
            &Some("99999999999".to_owned())
        );
        assert_eq!(
            &transaction.partner_account.bank_code,
            &Some("20111".to_owned())
        );
        assert_eq!(
            &transaction.partner_account.country_code,
            &Some("AT".to_owned())
        );

        assert_eq!(transaction.amount.value, -10000);
        assert_eq!(transaction.amount.precision, 2);
        assert_eq!(&transaction.amount.currency, "EUR");
    }
}
