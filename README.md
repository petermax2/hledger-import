# hledger Import

`hledger-import` imports data from bank export files (CSV, JSON, etc.) and converts the transactions into a `hledger` compatible format.

The following banks and formats are supported:

- Erste Bank JSON exports
- Revolut CSV exports
- card complete XML exports
- flatex CSV exports of settlement accounts

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

