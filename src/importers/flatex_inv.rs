use std::{
    fs::File,
    io::{BufReader, Read},
};

use lopdf::{content::Content, Document};
use serde::Deserialize;

use crate::HledgerImporter;
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
        known_codes: &std::collections::HashSet<String>,
    ) -> crate::error::Result<Vec<crate::hledger::output::Transaction>> {
        let texts = self.extract_text_from_pdf(input_file)?;

        let transaction = self.into_hledger(config, &texts)?;
        let code = transaction.code.as_ref().unwrap();

        if known_codes.contains(code) {
            Ok(vec![])
        } else {
            Ok(vec![transaction])
        }
    }

    fn output_title(&self) -> &'static str {
        "flatex import"
    }
}

impl FlatexPdfInvoiceImporter {
    fn into_hledger(&self, _config: &ImporterConfig, _texts: &Vec<String>) -> Result<Transaction> {
        todo!()
    }

    fn extract_text_from_pdf(&self, input_file: &std::path::Path) -> Result<Vec<String>> {
        let mut texts: Vec<String> = Vec::new();

        let file = match File::open(input_file) {
            Ok(f) => f,
            Err(_) => return Err(ImportError::InputFileRead(input_file.to_owned())),
        };

        let mut reader = BufReader::new(file);
        let mut pdf_content = Vec::new();

        match reader.read_to_end(&mut pdf_content) {
            Ok(_) => {}
            Err(_) => return Err(ImportError::InputFileRead(input_file.to_owned())),
        };

        let pdf_doc = Document::load_mem(&pdf_content)?;
        for (_, page_id) in pdf_doc.get_pages() {
            let page_content = pdf_doc.get_page_content(page_id)?;
            let content = Content::decode(&page_content)?;

            for operation in content.operations {
                for operand in operation.operands {
                    match operand {
                        lopdf::Object::String(ref text, _) => {
                            texts.push(Document::decode_text(None, text));
                        }
                        lopdf::Object::Array(array) => {
                            for obj in array {
                                if let lopdf::Object::String(ref text, _) = obj {
                                    texts.push(Document::decode_text(None, text));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(texts)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FlatexPdfConfig {
    pub account: String,
}
