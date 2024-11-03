# hledger Import

`hledger-import` imports data from bank export files (CSV, JSON, etc.) and converts the transactions into a `hledger` compatible format.

The following bank export formats are supported:

- Erste Bank JSON exports
- Revolut CSV exports
- card complete XML exports
- flatex CSV exports of settlement accounts
- flatex PDF invoice

## Compile and Run

Compile the project with cargo:

```sh
cargo build
```

or start directly:

```sh
cargo run -- --help
```

### Features

The importers are split into separate features. They can be enabled separately.

The following features are available:

- cardcomplete
- erste
- flatex
- revolut

All features are enabled per default.

If you want to have a custom build with a subset of importers, you must disable the default features.
The following examples builds `hledger-import` with only the _Revolut_ importer.

```sh
cargo build --no-default-features --features "revolut"
```

## Plans for the Future

- better documentation

