# Configuration File Format

This file is a work in progress.

## Location

The program `hledger-import` will by default look at the following location: `$HOME/.config/hledger-import/config.toml`.

The path to the configuration file can be set using the environment variable `HLEDGER_IMPORT_CONFIG`.

## File Format

The configuration file is written in TOML format.

### Top Level

#### ibans

`ibans` enumerates all of your bank accounts.
If `iban` matches, then `account` is used for the resulting hledger transaction.

### cards

TODO

### mapping

TODO

## Example File

The following example demonstrates the configuration file format:

```
ibans = [
  { iban = "AT000000000000000000", account = "Assets:Bank:Checking" },
  { iban = "AT000000000000000001", account = "Assets:Bank:Savings" },
]
cards = [
  { card = "Erste", account = "Liabilities:Credit Cards:Example VISA" },
]
mapping = [
  { search = "Grocery Store", account = "Expenses:Groceries", note = "Grocery shopping" },
]
creditor_and_debitor_mapping = []

[transfer_accounts]
bank = "Assets:Reconciliation:Bank transfers"
cash = "Assets:Reconciliation:Cash transfers"

[sepa]
creditors = [
  { creditor_id = "AT00ZZZ00000000000", account = "Expenses:Telecommunication", note = "Phone bill" },
]
mandates = [
  { mandate_id = "1234/56789", account = "Assets:Reconciliation:Bank transfers", note = "Credit card bill" },
]
```
