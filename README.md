# JSON-RPC

A simple, pragmatic implementation of [JSONRPC-2.0][] for [Rust][] that is transport agnostic and adheres strictly to the [specification][].

Nonblocking support is available using the `async` feature flag which requires [async-trait][], see the `async` example for usage:

```
cargo run --example hello-world
cargo run --example async
```

Dual-licensed under MIT and Apache-2.

[JSONRPC-2.0]: https://www.jsonrpc.org
[specification]: https://www.jsonrpc.org/specification
[Rust]: https://www.rust-lang.org/
[async-trait]: https://docs.rs/async-trait/0.1.42/async_trait/
