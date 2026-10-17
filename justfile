default: build

# build the Svelte frontend (single-file dist/index.html, embedded by cargo)
build-frontend:
	cd frontend && bun install && bun run build

# full release build: frontend + daemon binary
build: build-frontend
	cargo build --release

# type-check / lint both sides
check:
	cargo check
	cd frontend && bun run check

# vite dev server (proxies /ws to a running daemon on :4141)
dev-frontend:
	cd frontend && bun run dev

# build, then run the daemon (extra args are passed through)
run *args: build
	./target/release/mdrv-tuya {{args}}

test:
	cargo test
	cd frontend && bun test
