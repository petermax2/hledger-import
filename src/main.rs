use std::collections::HashSet;

use clap::{command, Parser};
use deduplication::get_hledger_codes;

pub mod deduplication;
pub mod error;
pub mod hledger;

/// bank data and credit card import programm for hledger accounting
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ImporterArgs {
    /// path to the input file to be imported to hledger
    #[arg(short, long)]
    input_file: String,

    /// try to avoid duplicate imports by reading in the known codes from hledger
    #[arg(short, long, default_value_t = false)]
    deduplicate: bool,
}

fn main() {
    let args = ImporterArgs::parse();

    let codes = if args.deduplicate {
        match get_hledger_codes() {
            Ok(codes) => codes,
            Err(e) => {
                eprintln!("[ERROR] {}", e);
                return;
            }
        }
    } else {
        HashSet::new()
    };

    dbg!(&args);
    dbg!(&codes);
}
