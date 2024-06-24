# hledger Import

This program is meant to be an import program to hledger. 
The `hledger-import` program converts bank export files to hledger transactions.

The following banks and formats are supported:

- Erste Bank JSON exports
- Revolut CSV exports
- card complete XML exports

**This tool is work in progress!**

## Compile and Run

Compile the project with cargo:

```sh
cargo build
```

or start directly:

```sh
cargo run -- --help
```

## Plans for the Future

The following banks and formats will be supported soon:

- flatex PDF invoice
- flatex CSV exports of settlement accounts
