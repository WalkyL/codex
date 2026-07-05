# Container Workflow

Use the local build image instead of building `openai-codex-fork` directly on the host.

## Default check

```powershell
.
\build-in-container.ps1
```

This runs:

```text
rustup toolchain install 1.95.0 --profile minimal
rustup component add clippy rustfmt rust-src --toolchain 1.95.0
cargo +1.95.0 check -p codex-cli
```

## Example: runtime snapshot

```powershell
.
\build-in-container.ps1 -Command "run -p codex-cli -- debug snapshot 'Inspect this workspace'"
```

## Notes

- mounts `D:\workspaces\openai-codex-fork\codex-rs` into `/workspace`
- mounts host Cargo `registry/` and `git/` caches into `/cache/cargo/`
- mounts host Rustup home into `/cache/rustup/`
- forwards the local proxy at `http://172.27.176.1:10808`
- enables `CARGO_NET_GIT_FETCH_WITH_CLI=true`
- installs minimal build prerequisites inside the container before running the command
- prewarms the Rust 1.95.0 toolchain and required components before invoking cargo
