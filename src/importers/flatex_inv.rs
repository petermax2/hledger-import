use serde::Deserialize;

use crate::HledgerImporter;

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
        _input_file: &std::path::Path,
        _config: &crate::config::ImporterConfig,
        _known_codes: &std::collections::HashSet<String>,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        todo!()
    }

    fn output_title(&self) -> &'static str {
        "flatex import"
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexPdfConfig {
    pub account: String,
}
