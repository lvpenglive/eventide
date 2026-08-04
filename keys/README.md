# License keys

- `license_public.pem` — vendor public key (safe to commit; also embedded in `eventide-license`).
- `license_private.pem` — **do not commit**. Used by `cargo run -p eventide-license -- issue …`.

Generate a new pair for your own fork, replace the embedded `PUBLIC_KEY_PEM` in `crates/eventide-license/src/lib.rs`, and keep the private key offline.
