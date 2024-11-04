.PHONY: fmt test integration-tests clippy
fmt:
	cargo fmt --all

test:
	cargo test --all-features --all
	cargo test --all --no-default-features
	cargo test --all --no-default-features --features async

integration-tests:
	# TODO
	cargo test

clippy:
	cargo clippy --all-targets --all-features --tests --benches -- -D warnings
	cargo clippy --all-targets --all-features --tests --benches --no-default-features -- -D warnings
	cargo clippy --all-targets --all-features --tests --benches --no-default-features --features async -- -D warnings

publish:
	cargo publish --allow-dirty --registry crates-io -p lyric-utils
	sleep 20

	cargo publish --allow-dirty --registry crates-io -p lyric-rpc
	sleep 20

	cargo publish --allow-dirty --registry crates-io -p lyric-wasm-runtime
	sleep 20

	cargo publish --allow-dirty --registry crates-io -p lyric
	sleep 20

publish-dry-run:
	cargo publish --dry-run --allow-dirty --registry crates-io -p lyric-utils
	sleep 20

	cargo publish --dry-run --allow-dirty --registry crates-io -p lyric-rpc
	sleep 20

	cargo publish --dry-run --allow-dirty --registry crates-io -p lyric-wasm-runtime
	sleep 20

	cargo publish --dry-run --allow-dirty --registry crates-io -p lyric
	sleep 20