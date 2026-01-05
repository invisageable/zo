# zo project task runner

# Run all pre-commit checks
pre-commit: fmt-check clippy test
  @echo "All pre-commit checks passed!"

# Format all code
fmt:
  cargo fmt --all

# Check formatting without modifying
fmt-check:
  cargo fmt --all -- --check

# Run clippy with warnings as errors
clippy:
  cargo clippy --all --all-targets -- -D warnings

# Run all tests
test:
  cargo test --all

# Build all targets
build:
  cargo build --all

# Clean build artifacts
clean:
  cargo clean

# === Benchmarks ===

# Run all cargo benchmarks
bench:
  cargo bench --all

eazy_run_bench:
  cargo bench -p eazy 

# Sync eazy benchmark reports to docs/ for GitHub Pages
eazy_build_bench_reports:
  uv run sources/tweener/eazy-tasks/build_bench_reports.py

# Run benchmarks and sync to docs (for GitHub Pages deployment)
eazy_publish_bench_reports: eazy_run_bench eazy_build_bench_reports
  @echo "Benchmarks published to docs/"

# Dry-run publish all eazy-* crates                                          
eazy_publish_dry:                                                            
  cargo publish -p eazy-core --dry-run                                       
  cargo publish -p eazy-derive --dry-run                                     
  cargo publish -p eazy-tweener --dry-run                                    
  cargo publish -p eazy-keyframe --dry-run                                   
  cargo publish -p eazy --dry-run  

# === Cross-platform testing (requires Docker) ===

# Run all checks in Linux container
test-linux:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "apt-get update && apt-get install -y libglib2.0-dev libgtk-3-dev libatk1.0-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev && \
           cargo fmt --all -- --check && \
           cargo clippy --all --all-targets -- -D warnings && \
           cargo test --all"

# Build for Linux (quick compile check)
build-linux:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "apt-get update && apt-get install -y libglib2.0-dev libgtk-3-dev libatk1.0-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev && \
           cargo build --all"

# Cross-compile check for Windows (no tests, just build)
build-windows:
  docker run --rm -v {{justfile_directory()}}:/workspace -w /workspace rust:latest \
    sh -c "rustup target add x86_64-pc-windows-gnu && \
           apt-get update && apt-get install -y mingw-w64 && \
           cargo build --all --target x86_64-pc-windows-gnu"

# Run full CI simulation locally
ci: fmt-check clippy test test-linux
  @echo "Full CI simulation passed!"

# Install the pre-commit hook
install-hooks:
  @echo '#!/bin/sh' > .git/hooks/pre-commit
  @echo 'just pre-commit' >> .git/hooks/pre-commit
  @chmod +x .git/hooks/pre-commit
  @echo "Pre-commit hook installed"

# === Version Management (cargo-workspaces) ===

# Bump patch version (0.1.0 -> 0.1.1) for all crates
release-patch:
  cargo ws version patch --no-git-push --yes

# Bump minor version (0.1.0 -> 0.2.0) for all crates
release-minor:
  cargo ws version minor --no-git-push --yes

# Bump major version (0.1.0 -> 1.0.0) for all crates
release-major:
  cargo ws version major --no-git-push --yes

# Set exact version for a crate: just set-version eazy 0.2.0
set-version crate version:
  cargo set-version -p {{crate}} {{version}}

# Bump a specific crate: just bump-crate eazy patch
bump-crate crate bump:
  cargo set-version -p {{crate}} --bump {{bump}}

# Bump all eazy-* crates together
bump-eazy bump:
  cargo set-version -p eazy-core --bump {{bump}}
  cargo set-version -p eazy-derive --bump {{bump}}
  cargo set-version -p eazy-tweener --bump {{bump}}
  cargo set-version -p eazy-keyframe --bump {{bump}}
  cargo set-version -p eazy --bump {{bump}}

# Bump all swisskit-* crates together
bump-swisskit bump:
  cargo set-version -p swisskit-case --bump {{bump}}
  cargo set-version -p swisskit-core --bump {{bump}}
  cargo set-version -p swisskit-fmt --bump {{bump}}
  cargo set-version -p swisskit-io --bump {{bump}}
  cargo set-version -p swisskit-renderer --bump {{bump}}
  cargo set-version -p swisskit-span --bump {{bump}}
  cargo set-version -p swisskit --bump {{bump}}

# Bump all zo-* crates together
bump-zo bump:
  #!/usr/bin/env sh
  for crate in $(cargo ws list | grep '^zo-'); do
    cargo set-version -p "$crate" --bump {{bump}}
  done
  cargo set-version -p zo --bump {{bump}}

# List all workspace crates and their versions
list_versions:
  cargo ws list -l

# Show what would change without applying
release_dry_run bump="patch":
  cargo ws version {{bump}} --no-git-push --dry-run

# === Publishing ===

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
  cargo publish -p swisskit-case
  cargo publish -p swisskit-core
  cargo publish -p swisskit-fmt
  cargo publish -p swisskit-io
  cargo publish -p swisskit-span
  cargo publish -p swisskit-renderer
  cargo publish -p swisskit

# === GitHub Releases ===

# Create a release tag: just release v0.1.0
release version:
  git tag -a {{version}} -m "Release {{version}}"
  git push origin {{version}}

# Delete a tag (if you made a mistake)
delete-tag version:
  git tag -d {{version}}
  git push origin :refs/tags/{{version}}

# List all tags
list-tags:
  git tag -l --sort=-v:refname
