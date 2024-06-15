use std::fmt::Display;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

/// helper structure that binds the currency/commodity to a given amount (e.g. 25.39 USD or 0.1 BTC)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmountAndCommodity {
    pub amount: BigDecimal,
    pub commodity: String,
}

impl Display for AmountAndCommodity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut amount = String::new();

        let format = num_format::CustomFormat::builder()
            .grouping(num_format::Grouping::Standard)
            .minus_sign("-")
            .separator(".")
            .build()
            .unwrap();

        let amount_str = self.amount.to_string();
        let mut parts = amount_str.split('.');

        // the part before the comma (before the decimal point)
        if let Some(before_decimal) = parts.next() {
            let mut buffer = num_format::Buffer::new();
            let before_decimal = before_decimal.parse::<i64>().unwrap();
            buffer.write_formatted(&before_decimal, &format);
            amount.push_str(buffer.as_str());
        }

        // the part after the comma (after the decimal point) - optional
        if let Some(after_decimal) = parts.next() {
            amount.push(',');
            amount.push_str(after_decimal);
        }

        write!(f, "{} {}", amount, &self.commodity)
    }
}

impl AmountAndCommodity {
    pub fn new(amount: BigDecimal, commodity: String) -> Self {
        Self { amount, commodity }
    }
}

/// hledger uses tags to identify transactions or postings.
/// Tags can hold values optionally.
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub value: Option<String>,
}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(value) = &self.value {
            write!(f, "{}: {}", &self.name, value)
        } else {
            write!(f, "{}:", &self.name)
        }
    }
}

impl Tag {
    pub fn new_date(date: &NaiveDate) -> Self {
        Self {
            name: String::from("date"),
            value: Some(date.format("%Y-%m-%d").to_string()),
        }
    }

    pub fn new_val(name: String, value: String) -> Self {
        Self {
            name,
            value: Some(value),
        }
    }

    pub fn new(name: String) -> Self {
        Self { name, value: None }
    }
}

/// The transaction (and posting) state indicates how the transaction is to be interpreted.
/// Cleared transactions are posted and confirmed by the bank (e.g. the transcation appears on the account statement).
/// Pending transactions are in an unclear state and might need further checking. Pending transactions are not verified.
/// Transactions in default state are registered in the accounting system and usually do not need any further verification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransactionState {
    #[default]
    Default,
    Cleared,
    Pending,
}

impl Display for TransactionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match &self {
            TransactionState::Default => " ",
            TransactionState::Cleared => "*",
            TransactionState::Pending => "!",
        };
        write!(f, "{}", c)
    }
}

/// In hledger a transaction is an accounting document that consists of a date and a set of postings on accounts.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: NaiveDate,
    pub code: Option<String>,
    pub payee: String,
    pub note: Option<String>,
    pub state: TransactionState,
    pub comment: Option<String>,
    pub tags: Vec<Tag>,
    pub postings: Vec<Posting>,
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let date = self.date.format("%Y-%m-%d").to_string();
        let mut result = format!("{} {}", &date, &self.state);
        if let Some(code) = &self.code {
            result = format!("{} ({})", &result, code);
        }
        result = format!("{} {}", &result, &self.payee);
        if let Some(note) = &self.note {
            result = format!("{} | {}", &result, note);
        }
        if let Some(comment) = &self.comment {
            result = format!("{}\n  ; {}", &result, comment);
        }
        self.tags.iter().for_each(|tag| {
            result = format!("{}\n  ; {}", &result, tag);
        });
        self.postings.iter().for_each(|p| {
            result = format!("{}\n{}", &result, p);
        });
        write!(f, "{}", &result)
    }
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account: String,
    pub amount: Option<AmountAndCommodity>,
    pub comment: Option<String>,
    pub tags: Vec<Tag>,
}

impl Display for Posting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut render = match &self.amount {
            Some(amount) => {
                let amount = amount.to_string();
                // 80 is the default line length - the amount should be aligned to the right (at position 80)
                let length_filler = 80 - 2 - amount.len() - 1;
                format!("  {:<w$} {}", &self.account, &amount, w = length_filler)
            }
            None => format!("  {}", &self.account),
        };
        if let Some(comment) = &self.comment {
            render = format!("{}\n  ; {}", &render, comment);
        }
        self.tags.iter().for_each(|tag| {
            render = format!("{}\n  ; {}", &render, tag);
        });
        write!(f, "{}", &render)
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, vec};

    use super::*;

    #[test]
    fn state_to_str() {
        let result = TransactionState::Cleared.to_string();
        assert_eq!(result, "*");
        let result = TransactionState::Pending.to_string();
        assert_eq!(result, "!");
        let result = TransactionState::Default.to_string();
        assert_eq!(result, " ");
        let result = TransactionState::default().to_string();
        assert_eq!(result, " ");
    }

    #[test]
    fn default_state() {
        let result = TransactionState::default();
        assert_eq!(result, TransactionState::Default);
    }

    #[test]
    fn tag_to_str() {
        let tag = Tag::new(String::from("lunch"));
        let result = tag.to_string();
        assert_eq!(result, "lunch:");

        let tag = Tag::new_val(String::from("valuation"), String::from("2024-11-22"));
        let result = tag.to_string();
        assert_eq!(result, "valuation: 2024-11-22");

        let date = NaiveDate::from_ymd_opt(2024, 11, 20).unwrap();
        let tag = Tag::new_date(&date);
        let result = tag.to_string();
        assert_eq!(result, "date: 2024-11-20");
    }

    #[test]
    fn amount_to_str() {
        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("-299101.12").unwrap(),
            commodity: String::from("EUR"),
        };
        let result = amount.to_string();
        assert_eq!(result, "-299.101,12 EUR");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("1799361.99").unwrap(),
            commodity: String::from("EUR"),
        };
        let result = amount.to_string();
        assert_eq!(result, "1.799.361,99 EUR");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("0.12345678").unwrap(),
            commodity: String::from("BTC"),
        };
        let result = amount.to_string();
        assert_eq!(result, "0,12345678 BTC");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("22").unwrap(),
            commodity: String::from("GLD"),
        };
        let result = amount.to_string();
        assert_eq!(result, "22 GLD");
    }

    #[test]
    fn posting_to_str() {
        let posting = Posting {
            account: String::from("Assets:Cash"),
            amount: Some(AmountAndCommodity::new(
                BigDecimal::from_str("-11.44").unwrap(),
                "EUR".to_owned(),
            )),
            comment: None,
            tags: vec![
                Tag::new("lunch".to_owned()),
                Tag::new_val("valuation".to_owned(), "2024-05-02".to_owned()),
            ],
        };
        let result = posting.to_string();
        assert_eq!(result, "  Assets:Cash                                                         -11,44 EUR\n  ; lunch:\n  ; valuation: 2024-05-02");

        let posting = Posting {
            account: String::from("Expenses:Groceries"),
            amount: None,
            comment: None,
            tags: vec![],
        };
        let result = posting.to_string();
        assert_eq!(result, "  Expenses:Groceries");

        let posting = Posting {
            account: String::from("Expenses:Groceries"),
            amount: None,
            comment: Some("test comment".to_owned()),
            tags: vec![],
        };
        let result = posting.to_string();
        assert_eq!(result, "  Expenses:Groceries\n  ; test comment");
    }

    #[test]
    fn transaction_to_str() {
        let t = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 11, 22).unwrap(),
            code: Some("ABC123".to_owned()),
            payee: "Test".to_owned(),
            note: Some("Note".to_owned()),
            state: TransactionState::Cleared,
            comment: Some("comment".to_owned()),
            tags: vec![],
            postings: vec![],
        };
        let result = t.to_string();
        assert_eq!(result, "2024-11-22 * (ABC123) Test | Note\n  ; comment");

        let t = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 11, 22).unwrap(),
            code: Some("ABC123".to_owned()),
            payee: "Test".to_owned(),
            note: Some("Note".to_owned()),
            state: TransactionState::Cleared,
            comment: Some("comment".to_owned()),
            tags: vec![
                Tag::new("lunch".to_owned()),
                Tag::new_val("foo".to_owned(), "bar".to_owned()),
            ],
            postings: vec![],
        };
        let result = t.to_string();
        assert_eq!(
            result,
            "2024-11-22 * (ABC123) Test | Note\n  ; comment\n  ; lunch:\n  ; foo: bar"
        );

        let t = Transaction {
            date: NaiveDate::from_ymd_opt(2024, 11, 22).unwrap(),
            code: None,
            payee: "Payer".to_owned(),
            note: None,
            state: TransactionState::Pending,
            comment: None,
            tags: vec![],
            postings: vec![],
        };
        let result = t.to_string();
        assert_eq!(result, "2024-11-22 ! Payer");
    }

    #[test]
    fn full_transaction_to_str() {
        let t = Transaction {
            date: NaiveDate::from_ymd_opt(2020, 6, 18).unwrap(),
            code: Some("123-XYZ-321".to_owned()),
            payee: "Store".to_owned(),
            note: Some("Bought something".to_owned()),
            state: TransactionState::Cleared,
            comment: Some("this is a test".to_owned()),
            tags: vec![],
            postings: vec![
                Posting {
                    account: "Assets:Cash".to_owned(),
                    amount: Some(AmountAndCommodity::new(
                        BigDecimal::from_str("-2799.97").unwrap(),
                        "EUR".to_owned(),
                    )),
                    comment: None,
                    tags: vec![],
                },
                Posting {
                    account: "Expenses:Test".to_owned(),
                    amount: None,
                    comment: Some("Some test".to_owned()),
                    tags: vec![],
                },
            ],
        };
        let result = t.to_string();
        assert_eq!(result, "2020-06-18 * (123-XYZ-321) Store | Bought something\n  ; this is a test\n  Assets:Cash                                                      -2.799,97 EUR\n  Expenses:Test\n  ; Some test");

        let t = Transaction {
            date: NaiveDate::from_ymd_opt(2020, 6, 18).unwrap(),
            code: None,
            payee: "Store".to_owned(),
            note: Some("Bought something".to_owned()),
            state: TransactionState::Cleared,
            comment: Some("this is a test".to_owned()),
            tags: vec![],
            postings: vec![
                Posting {
                    account: "Assets:Cash".to_owned(),
                    amount: Some(AmountAndCommodity::new(
                        BigDecimal::from_str("-2799.97").unwrap(),
                        "EUR".to_owned(),
                    )),
                    comment: None,
                    tags: vec![],
                },
                Posting {
                    account: "Expenses:Test".to_owned(),
                    amount: None,
                    comment: Some("Some test".to_owned()),
                    tags: vec![],
                },
            ],
        };
        let result = t.to_string();
        assert_eq!(result, "2020-06-18 * Store | Bought something\n  ; this is a test\n  Assets:Cash                                                      -2.799,97 EUR\n  Expenses:Test\n  ; Some test");
    }
}
