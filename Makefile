.PHONY: build check test fmt clippy deny clean

build:
	cargo build --release

# The CI gate, locally: fmt, clippy -D warnings, tests, supply chain.
check: fmt clippy test deny

test:
	cargo test --workspace

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

deny:
	cargo deny --config deny.toml check --hide-inclusion-graph licenses advisories sources

clean:
	cargo clean
