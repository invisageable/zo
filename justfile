# zo project task runner

# Run all pre-commit checks
pre-commit: fmt-check clippy test
  @echo "✅ All pre-commit checks passed!"

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

# Install the pre-commit hook
install-hooks:
  @echo '#!/bin/sh' > .git/hooks/pre-commit
  @echo 'just pre-commit' >> .git/hooks/pre-commit
  @chmod +x .git/hooks/pre-commit
  @echo "✓ Pre-commit hook installed"
