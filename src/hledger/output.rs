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
        write!(f, "{} {}", self.amount, &self.commodity)
    }
}

impl AmountAndCommodity {
    pub fn new(amount: BigDecimal, commodity: String) -> Self {
        Self { amount, commodity }
    }
}

/// hledger uses tags to identify transactions or postings.
/// Tags can hold values optionally.
#[derive(Debug, Clone, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
            result = format!("{}\n    ; {}", &result, comment);
        }
        self.tags.iter().for_each(|tag| {
            result = format!("{}\n    ; {}", &result, tag);
        });
        self.postings.iter().for_each(|p| {
            result = format!("{}\n{}", &result, p);
        });
        write!(f, "{}", &result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
                format!("    {}     {}", &self.account, &amount)
            }
            None => format!("    {}", &self.account),
        };
        if let Some(comment) = &self.comment {
            render = format!("{}\n    ; {}", &render, comment);
        }
        self.tags.iter().for_each(|tag| {
            render = format!("{}\n    ; {}", &render, tag);
        });
        write!(f, "{}", &render)
    }
}

#[derive(Debug)]
pub struct HeaderComment<'a> {
    pub title: &'a str,
}

impl<'a> HeaderComment<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title }
    }
}

impl Display for HeaderComment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let asterisk_line: String = "*".repeat(78);
        let date_time = chrono::Local::now().to_rfc2822();
        let gap: String = " ".repeat(80 - self.title.len() - date_time.len() - 2);
        write!(
            f,
            "; {}\n; {}{}{}\n; {}",
            asterisk_line, self.title, gap, date_time, asterisk_line
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, vec};

    use bigdecimal::FromPrimitive;

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
        assert_eq!(result, "-299101.12 EUR");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("1799361.99").unwrap(),
            commodity: String::from("EUR"),
        };
        let result = amount.to_string();
        assert_eq!(result, "1799361.99 EUR");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("0.12345678").unwrap(),
            commodity: String::from("BTC"),
        };
        let result = amount.to_string();
        assert_eq!(result, "0.12345678 BTC");

        let amount = AmountAndCommodity {
            amount: BigDecimal::from_str("22").unwrap(),
            commodity: String::from("GLD"),
        };
        let result = amount.to_string();
        assert_eq!(result, "22 GLD");

        let a = AmountAndCommodity {
            amount: BigDecimal::from_str("10").unwrap(),
            commodity: "EUR".to_owned(),
        };
        assert_eq!(a.to_string(), "10 EUR");

        let a = AmountAndCommodity {
            amount: BigDecimal::from_str("12.1").unwrap(),
            commodity: "USD".to_owned(),
        };
        assert_eq!(a.to_string(), "12.1 USD");
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
        assert_eq!(
            result,
            "    Assets:Cash     -11.44 EUR\n    ; lunch:\n    ; valuation: 2024-05-02"
        );

        let posting = Posting {
            account: String::from("Expenses:Groceries"),
            amount: None,
            comment: None,
            tags: vec![],
        };
        let result = posting.to_string();
        assert_eq!(result, "    Expenses:Groceries");

        let posting = Posting {
            account: String::from("Expenses:Groceries"),
            amount: None,
            comment: Some("test comment".to_owned()),
            tags: vec![],
        };
        let result = posting.to_string();
        assert_eq!(result, "    Expenses:Groceries\n    ; test comment");
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
        assert_eq!(result, "2024-11-22 * (ABC123) Test | Note\n    ; comment");

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
            "2024-11-22 * (ABC123) Test | Note\n    ; comment\n    ; lunch:\n    ; foo: bar"
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
        assert_eq!(
            result,
            "2020-06-18 * (123-XYZ-321) Store | Bought something\n    ; this is a test\n    Assets:Cash     -2799.97 EUR\n    Expenses:Test\n    ; Some test"
        );

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
        assert_eq!(
            result,
            "2020-06-18 * Store | Bought something\n    ; this is a test\n    Assets:Cash     -2799.97 EUR\n    Expenses:Test\n    ; Some test"
        );
    }

    #[test]
    fn display_minus_one_cent() {
        let amount = AmountAndCommodity {
            amount: BigDecimal::from_i64(-1).unwrap() / 100,
            commodity: "EUR".to_owned(),
        };
        let result = amount.to_string();
        assert_eq!(result, "-0.01 EUR");
    }
}
