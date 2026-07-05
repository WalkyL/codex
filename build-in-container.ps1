param(
  [string]$Image = 'localhost/product-build-runner:latest',
  [string]$Proxy = 'http://172.27.176.1:10808',
  [string]$Command = 'check -p codex-cli',
  [string]$RustToolchain = '1.95.0',
  [string]$RustComponents = 'clippy rustfmt rust-src'
)

$ErrorActionPreference = 'Stop'

$repoRoot = 'D:\workspaces\openai-codex-fork\codex-rs'
$cargoCache = 'D:\Rust\cargo'
$cargoRegistry = Join-Path $cargoCache 'registry'
$cargoGit = Join-Path $cargoCache 'git'
$rustupHome = 'D:\Rust\rustup'

if (-not (Test-Path -LiteralPath $repoRoot)) {
  throw "Repo root not found: $repoRoot"
}

if (-not (Test-Path -LiteralPath $cargoRegistry)) {
  throw "Cargo registry cache not found: $cargoRegistry"
}

if (-not (Test-Path -LiteralPath $cargoGit)) {
  throw "Cargo git cache not found: $cargoGit"
}

if (-not (Test-Path -LiteralPath $rustupHome)) {
  throw "Rustup home not found: $rustupHome"
}

podman run --rm `
  -v "${repoRoot}:/workspace" `
  -v "${cargoRegistry}:/cache/cargo/registry" `
  -v "${cargoGit}:/cache/cargo/git" `
  -v "${rustupHome}:/cache/rustup" `
  -v "C:\Users\Walky\.codex\config.toml:/root/.codex/config.toml" `
  -v "C:\Users\Walky\.codex\auth.json:/root/.codex/auth.json:ro" `
  -w /workspace `
  -e "HTTP_PROXY=$Proxy" `
  -e "HTTPS_PROXY=$Proxy" `
  -e "http_proxy=$Proxy" `
  -e "https_proxy=$Proxy" `
  -e "NO_PROXY=localhost,127.0.0.1" `
  -e "CARGO_HOME=/cache/cargo" `
  -e "RUSTUP_HOME=/cache/rustup" `
  -e "CARGO_NET_GIT_FETCH_WITH_CLI=true" `
  -e "RUSTUP_DIST_SERVER=https://static.rust-lang.org" `
  -e "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup" `
  $Image `
  env -i HTTP_PROXY="$Proxy" HTTPS_PROXY="$Proxy" http_proxy="$Proxy" https_proxy="$Proxy" NO_PROXY="localhost,127.0.0.1" CARGO_HOME="/cache/cargo" RUSTUP_HOME="/cache/rustup" PATH="/cache/cargo/bin:/usr/bin:/usr/local/bin" RUSTUP_DIST_SERVER="https://static.rust-lang.org" RUSTUP_UPDATE_ROOT="https://static.rust-lang.org/rustup" sh -lc "mkdir -p /cache/cargo/registry /cache/cargo/git /cache/rustup && apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev python3 ca-certificates >/dev/null && rustup toolchain install $RustToolchain --profile minimal && rustup component add $RustComponents --toolchain $RustToolchain && cargo +$RustToolchain $Command"
