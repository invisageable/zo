default:
  @just --list

# Install typos.
[group('setup')]
setup_typos:
  @cargo install typos-cli

# Install `uv`.
[group('setup')]
setup_uv:
  curl -LsSf https://astral.sh/uv/install.sh | sh

# Install the dev environment.
[parallel]
[group('setup')]
setup: setup_typos setup_uv

# Install git hooks via lefthook.
install_hooks:
  lefthook install

lefthook:
  lefthook run pre-commit

# Run all pre-commit checks
pre-commit: lefthook

# Run typos check
typos:
  typos --format=brief

# Fix typos in-place
typos_fix:
  typos --write-changes

# Format all code
fmt:
  cargo fmt --all

# Run clippy with warnings as errors
clippy:
  cargo clippy --all --all-targets -- -D warnings

[group('lint')] 
[parallel]                                                                          
lint: typos fmt clippy

# Run all cargo benchmarks
bench:
  cargo bench --all

# Run both test suites in parallel
[parallel]
[group('test')]
test_all: test zo_test zo_test_runner

# Run all tests
[group('test')]
test:
  cargo nextest run --workspace --all-features

# Run tests for a specific crate
[group('test')]
test_crate crate:
  cargo nextest run -p {{crate}}

# Run a specific test by name
[group('test')]
test_filter filter:
  cargo nextest run -E 'test({{filter}})'

# Build all targets
build:
  cargo build --all

# Clean build artifacts
clean:
  cargo clean

# Check a specific crate (faster than build)
check crate:
  cargo check -p {{crate}}

# Install/upgrade zo via the install script
[group('zo')]
zo_install:
  sh tasks/zo-install.sh

# Build the zo compiler binary
[group("zo")]
zo_build_compiler:
  cargo build --bin zo

# Build a zo program
[group("zo")]
zo_build program:
  cargo run --bin zo -- build {{program}}

# Run a zo program
[group("zo")]
zo_run program:
  cargo run --bin zo -- run {{program}}

# Run all zo crates tests
[group('zo')]
[group('test')]
zo_test:
  cargo nextest run --workspace -E 'package(/^zo/)' --all-features

# Run zo program integration tests
[group("zo")]
[group('test')]
zo_test_runner:
  cargo run --bin zo-test-runner

# Run zo program tests (quick — skip build-pass)
[group("zo")]
[group('test')]
zo_test_quick:
  cargo run --bin zo-test-runner -- --quick

# Run zo compiler benchmark
[group('zo')]
zo_bench program:
  cargo build --release --bin zo && cargo run --release -p zo-benches -- {{program}}

# Run zo benchmark with regression check (strict — fails on regression)
[group('zo')]
zo_bench_check program:
  cargo build --release --bin zo && cargo run --release -p zo-benches -- {{program}} --strict

# Quick zo benchmark for pre-commit (hello only, 3 runs, zo only)
[group('zo')]
zo_bench_quick:
  cargo build --release --bin zo && cargo run --release -p zo-benches -- hello --quick --strict

# Update zo benchmark baseline
[group('zo')]
zo_bench_update:
  cargo build --release --bin zo && cargo run --release -p zo-benches -- all --update-baseline

# Run `eazy` bench
eazy_run_bench:
  cargo bench -p eazy 

# Sync eazy benchmark reports to docs/ for GitHub Pages
eazy_build_bench_reports:
  uv run sources/tweener/eazy-tasks/build_bench_reports.py

# Run benchmarks and sync to docs (for GitHub Pages deployment)
[parallel]
eazy_publish_bench_reports: eazy_run_bench eazy_build_bench_reports
  @echo "Benchmarks published to docs/"

# Dry-run publish all eazy-* crates                                          
eazy_publish_dry:                                                            
  cargo publish -p eazy-core --dry-run                                       
  cargo publish -p eazy-derive --dry-run                                     
  cargo publish -p eazy-tweener --dry-run                                    
  cargo publish -p eazy-keyframe --dry-run                                   
  cargo publish -p eazy --dry-run  

# Cross-platform testing (requires Docker)

# Run all checks in Linux container
test_linux:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "apt-get update && apt-get install -y libglib2.0-dev libgtk-3-dev libatk1.0-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev && \
           cargo fmt --all -- --check && \
           cargo clippy --all --all-targets -- -D warnings && \
           cargo test --all"

# Build for Linux (quick compile check)
build_linux:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "apt-get update && apt-get install -y libglib2.0-dev libgtk-3-dev libatk1.0-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev && \
           cargo build --all"

# Cross-compile check for Windows (no tests, just build)
build_windows:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "rustup target add x86_64-pc-windows-gnu && \
           apt-get update && apt-get install -y mingw-w64 && \
           cargo build --all --target x86_64-pc-windows-gnu"

# Run full CI simulation locally
[parallel]
ci: fmt clippy test test_linux
  @echo "Full CI simulation passed!"

# Version Management (cargo-workspaces)

# Bump patch version (0.1.0 -> 0.1.1) for all crates
release_patch:
  cargo ws version patch --no-git-push --yes

# Bump minor version (0.1.0 -> 0.2.0) for all crates
release_minor:
  cargo ws version minor --no-git-push --yes

# Bump major version (0.1.0 -> 1.0.0) for all crates
release_major:
  cargo ws version major --no-git-push --yes

# Set exact version for a crate: just set_version eazy 0.2.0
set_version crate version:
  cargo set-version -p {{crate}} {{version}}

# Bump a specific crate: just bump_crate eazy patch
bump_crate crate bump:
  cargo set-version -p {{crate}} --bump {{bump}}

# Bump all eazy-* crates together
bump_eazy bump:
  #!/usr/bin/env sh
  for crate in $(cargo ws list | grep '^eazy-'); do
    cargo set-version -p "$crate" --bump {{bump}}
  done
  cargo set-version -p eazy --bump {{bump}}

# Bump all swisskit-* crates together
bump_swisskit bump:
  #!/usr/bin/env sh
  for crate in $(cargo ws list | grep '^swisskit-'); do
    cargo set-version -p "$crate" --bump {{bump}}
  done
  cargo set-version -p swisskit --bump {{bump}}

# Bump zo + fret (one ecosystem, one version)
[group('zo')]
bump bump:
  cargo set-version -p zo --bump {{bump}}

# List all workspace crates and their versions
list_versions:
  cargo ws list -l

# Show what would change without applying
release_dry_run bump="patch":
  cargo ws version {{bump}} --no-git-push --dry-run

# Publish a single crate: just publish eazy
publish crate:
  cargo publish -p {{crate}}

# Dry-run publish (verify without uploading)
publish_dry crate:
  cargo publish -p {{crate}} --dry-run

# Publish all eazy-* crates (in dependency order)
publish_eazy:
  cargo publish -p eazy-core
  cargo publish -p eazy-derive
  cargo publish -p eazy-tweener
  cargo publish -p eazy-keyframe
  cargo publish -p eazy

# Publish all swisskit-* crates (in dependency order)
publish_swisskit:
  cargo publish -p swisskit-core
  cargo publish -p swisskit-renderer
  cargo publish -p swisskit

# Dry-run publish all fret-* crates
fret_publish_dry:
  cargo publish -p fret-tokens --dry-run
  cargo publish -p fret-types --dry-run
  cargo publish -p fret-tokenizer --dry-run
  cargo publish -p fret-parser --dry-run
  cargo publish -p fret-pipeline --dry-run
  cargo publish -p fret-driver --dry-run
  cargo publish -p fret --dry-run

# Publish all fret-* crates (in dependency order)
publish_fret:
  cargo publish -p fret-tokens
  cargo publish -p fret-types
  cargo publish -p fret-tokenizer
  cargo publish -p fret-parser
  cargo publish -p fret-pipeline
  cargo publish -p fret-driver
  cargo publish -p fret

# Create a release tag: just release 0.1.0
release version:
  git tag -a {{version}} -m "zo {{version}}"
  git push origin {{version}}

# Delete a tag (if you made a mistake): just delete_tag 0.1.0
delete_tag version:
  git tag -d {{version}}
  git push origin :refs/tags/{{version}}

# List all tags
list_tags:
  git tag -l --sort=-v:refname
