# Web frontend

API types are generated from Rust using `ts-rs` into `web/src/bindings/`.
Regenerate them from the repository root after changing the Rust types:

```sh
cargo test export_bindings
```

Import the overview response type with
`import type { Overview } from './bindings/Overview'` from a file in `web/src/`.

For development, run these commands from the repository root in separate terminals:

```sh
npm --prefix web run dev
```

```sh
cargo install cargo-watch # Once
./dev.sh /path/to/project
```

Open http://127.0.0.1:3000. Axum uses `axum-vite` to proxy frontend HTTP
requests to Vite on port 5173.
Vite must be running separately; no frontend build is needed in this mode.
The script rebuilds and restarts the Rust server when its source or Cargo files change.
Development mode requires a debug Rust build (omit `--release`).

For production, build the frontend and serve its static files:

```sh
npm --prefix web run build
cargo run -- --project . web
```
