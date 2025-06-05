use std::collections::HashSet;

use crate::hledger::deduplication::get_hledger_codes;
use crate::hledger::output::Transaction;
use clap::{Parser, ValueEnum, command};
use config::ImporterConfig;
use error::Result;
use hledger::{format::hledger_format, output::HeaderComment};

pub mod config;
pub mod error;
pub mod hasher;
pub mod hledger;
pub mod importers;

pub trait HledgerImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &ImporterConfig,
    ) -> Result<Vec<Transaction>>;

    fn output_title(&self) -> &'static str;
}

#[derive(Debug, Clone, ValueEnum)]
enum Importer {
    /// Erste Bank JSON export file
    #[cfg(feature = "erste")]
    Erste,

    /// Revolut CSV export file
    #[cfg(feature = "revolut")]
    Revolut,

    /// Cardcomplete XML export file
    #[cfg(feature = "cardcomplete")]
    Cardcomplete,

    /// Flatex CSV export file (of settlement accounts)
    #[cfg(feature = "flatex")]
    FlatexCSV,

    /// Flatex PDF invoice (of stock exchange transactions)
    #[cfg(feature = "flatex")]
    FlatexPDF,

    /// PayPal TXT (tab-separated) transaction list
    #[cfg(feature = "paypal")]
    Paypal,
}

impl From<Importer> for Box<dyn HledgerImporter> {
    fn from(val: Importer) -> Self {
        match val {
            #[cfg(feature = "erste")]
            Importer::Erste => Box::new(importers::erste::HledgerErsteJsonImporter::new()),
            #[cfg(feature = "revolut")]
            Importer::Revolut => Box::new(importers::revolut::RevolutCsvImporter::new()),
            #[cfg(feature = "cardcomplete")]
            Importer::Cardcomplete => {
                Box::new(importers::cardcomplete::CardcompleteXmlImporter::new())
            }
            #[cfg(feature = "flatex")]
            Importer::FlatexCSV => Box::new(importers::flatex_csv::FlatexCsvImport::new()),
            #[cfg(feature = "flatex")]
            Importer::FlatexPDF => Box::new(importers::flatex_inv::FlatexPdfInvoiceImporter::new()),
            #[cfg(feature = "paypal")]
            Importer::Paypal => Box::new(importers::paypal::PaypalPdfImporter::new()),
        }
    }
}

/// bank data and credit card import programm for hledger accounting
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ImporterArgs {
    /// path to the input file to be imported to hledger
    #[arg(short, long)]
    input_file: std::path::PathBuf,

    /// file type of given input file
    #[arg(short = 't', long)]
    file_type: Importer,

    /// try to avoid duplicate imports by reading in the known codes from hledger
    #[arg(short, long, default_value_t = false)]
    deduplicate: bool,
}

fn get_known_transaction_codes(
    deduplicate: bool,
    config: &ImporterConfig,
    transactions: &Vec<Transaction>,
) -> Result<HashSet<String>> {
    let codes = if deduplicate {
        let mut accounts: HashSet<String> = HashSet::with_capacity(1);
        if let Some(deduplication_accounts) = &config.deduplication_accounts {
            if !deduplication_accounts.is_empty() {
                for t in transactions {
                    for p in &t.postings {
                        if !accounts.contains(&p.account)
                            && deduplication_accounts.contains(&p.account)
                        {
                            accounts.insert(p.account.clone());
                        }
                    }
                }
            }
        }

        get_hledger_codes(&config.hledger, accounts)?
    } else {
        HashSet::new()
    };
    Ok(codes)
}

fn run_importer() -> Result<()> {
    let args = ImporterArgs::parse();
    let config = ImporterConfig::load()?;

    let importer: Box<dyn HledgerImporter> = args.file_type.into();
    let transactions = importer.parse(&args.input_file, &config)?;

    let codes = get_known_transaction_codes(args.deduplicate, &config, &transactions)?;

    let transactions: Vec<String> = transactions
        .iter()
        .filter(|t| {
            // handle deduplication - if no transaction code is provided, the transaction must be considered to be unique
            match &t.code {
                Some(code) => !codes.contains(code),
                None => true,
            }
        })
        .map(|t| t.to_string())
        .collect();
    let transactions = transactions.join("\n");

    let transactions = hledger_format(
        &config.hledger,
        &transactions,
        &config.commodity_formatting_rules,
    )?;

    let title = HeaderComment::new(importer.output_title());

    println!("{}\n{}", title, transactions);

    Ok(())
}

fn main() {
    if let Err(e) = run_importer() {
        eprintln!("[ERROR] {}", e);
    }
}
