use std::collections::HashSet;

use clap::{command, Parser, ValueEnum};
use config::ImporterConfig;
use deduplication::get_hledger_codes;
use error::Result;
use hledger::Transaction;

pub mod config;
pub mod deduplication;
pub mod error;
pub mod hledger;
pub mod importers;

pub trait HledgerImporter {
    fn parse(
        &self,
        input_file: &std::path::Path,
        config: &ImporterConfig,
        known_codes: &HashSet<String>,
    ) -> Result<Vec<Transaction>>;
}

#[derive(Debug, Clone, ValueEnum)]
enum Importer {
    //#[cfg(feature = "erste-json")]
    /// Erste Bank JSON export file
    Erste,
}

impl From<Importer> for Box<dyn HledgerImporter> {
    fn from(val: Importer) -> Self {
        match val {
            Importer::Erste => Box::new(importers::erste::HledgerErsteJsonImporter::new()),
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

fn main() {
    let args = ImporterArgs::parse();

    let config = match ImporterConfig::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[ERROR] {}", e);
            return;
        }
    };

    let codes = if args.deduplicate {
        match get_hledger_codes(&config.hledger) {
            Ok(codes) => codes,
            Err(e) => {
                eprintln!("[ERROR] {}", e);
                return;
            }
        }
    } else {
        HashSet::new()
    };

    let importer: Box<dyn HledgerImporter> = args.file_type.into();
    match importer.parse(&args.input_file, &config, &codes) {
        Ok(transactions) => {
            transactions.iter().for_each(|t| println!("{}\n", t));
        }
        Err(e) => {
            eprintln!("[ERROR] {}", e);
        }
    }
}
