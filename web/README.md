# Web frontend

## Development

API types are generated from Rust using `ts-rs` into `web/src/bindings/`.
Regenerate them from the repository root after changing the Rust types:

```sh
$ cargo test export_bindings
```

To run, run these commands from the repository root in separate terminals:

```sh
$ npm --prefix web run dev
```

```sh
$ cargo install cargo-watch # once
$ ./dev.sh /path/to/project
```

## Deploy

See `/deploy.sh`.
