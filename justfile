set shell := ["bash", "-euo", "pipefail", "-c"]

toolchain_dir := "target/rust-toolchain"
clippy_report := toolchain_dir + "/clippy.json"
coverage_report := toolchain_dir + "/lcov.info"

# ---------------------------------------------------------
# Fast developer workflow
# ---------------------------------------------------------

fmt:
    cargo fmt --all -- --check


check:
    cargo check --workspace --all-targets --all-features


clippy:
    cargo clippy \
        --workspace \
        --all-targets \
        --all-features \
        -- \
        -D warnings


test:
    cargo nextest run \
        --workspace \
        --all-features

    cargo test \
        --doc \
        --workspace \
        --all-features


# Fast developer validation.
quick: fmt check clippy test


# Alias developers/agents are expected to use most often.
doctor-check: quick


# ---------------------------------------------------------
# Dependency verification
# ---------------------------------------------------------

deps:
    cargo deny check

    cargo shear --deny-warnings


# ---------------------------------------------------------
# Cargo feature validation
# ---------------------------------------------------------

features:
    cargo hack check \
        --workspace \
        --each-feature \
        --no-dev-deps


features-deep:
    cargo hack check \
        --workspace \
        --feature-powerset \
        --depth 2 \
        --no-dev-deps


# ---------------------------------------------------------
# Comprehensive LOCAL Rust-Toolchain
# ---------------------------------------------------------
#
# NOTE:
# Plain cargo check is intentionally omitted here because
# Clippy already performs compilation/checking as part of its
# analysis.
#
# `rust-tc check` remains useful during normal development because
# it provides the fastest compiler-only feedback path.
#

doctor: fmt clippy test deps features
    @echo
    @echo "Rust-Toolchain: PASS"


# ---------------------------------------------------------
# Sonar report generation
# ---------------------------------------------------------

prepare-toolchain-dir:
    mkdir -p {{toolchain_dir}}


# Run Clippy exactly ONCE for the Sonar pipeline.
#
# SonarQube will import this report and MUST NOT execute its own
# Clippy analysis.
clippy-report: prepare-toolchain-dir
    cargo clippy \
        --workspace \
        --all-targets \
        --all-features \
        --message-format=json \
        -- \
        -D warnings \
        > {{clippy_report}}


# Run normal test binaries exactly ONCE while simultaneously
# collecting coverage.
coverage: prepare-toolchain-dir
    cargo llvm-cov \
        --lcov \
        --output-path {{coverage_report}} \
        nextest \
        --workspace \
        --all-features


# Nextest does not currently execute doctests, so this is a
# distinct/non-duplicated test category.
doctest:
    cargo test \
        --doc \
        --workspace \
        --all-features


# ---------------------------------------------------------
# SonarQube pipeline
# ---------------------------------------------------------
#
# IMPORTANT:
#
# Do NOT call `doctor` here.
#
# Doing that would execute Clippy and tests once during doctor
# and again while generating the Clippy/coverage reports.
#
# The Sonar pipeline deliberately has its own execution graph.
#
# Note: SonarQube CLI (`sonar`) is for auth/issue listing/agent
# hooks. Full project upload uses SonarScanner CLI (`sonar-scanner`).
#

sonar-prep: fmt clippy-report deps features coverage doctest
    @echo
    @echo "Rust-Toolchain Sonar reports prepared."


sonar: sonar-prep
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -f .env.sonar ]]; then
      set -a
      # shellcheck disable=SC1091
      source .env.sonar
      set +a
    fi
    : "${SONAR_HOST_URL:?Set SONAR_HOST_URL or create .env.sonar}"
    : "${SONAR_TOKEN:?Set SONAR_TOKEN or create .env.sonar}"
    sonar-scanner


# ---------------------------------------------------------
# SemVer validation
# ---------------------------------------------------------
#
# Intended primarily for library/public API crates.
#

semver package baseline="origin/main":
    cargo semver-checks \
        --package "{{package}}" \
        --baseline-rev "{{baseline}}"


# ---------------------------------------------------------
# Deep verification
# ---------------------------------------------------------

mutants:
    cargo mutants


miri:
    cargo +nightly miri test


fuzz target:
    cargo +nightly fuzz run "{{target}}"


deep: features-deep
    @echo
    @echo "Feature powerset validation complete."
    @echo "Run mutation, Miri and fuzz checks selectively:"
    @echo "  rust-tc mutants"
    @echo "  rust-tc miri"
    @echo "  rust-tc fuzz <target>"


# ---------------------------------------------------------
# Convenience
# ---------------------------------------------------------

clean-toolchain:
    rm -rf {{toolchain_dir}}


toolchain-help:
    @echo "Rust-Toolchain"
    @echo
    @echo "  rust-tc check         Fast compiler check"
    @echo "  rust-tc quick         Fast developer quality gate"
    @echo "  rust-tc doctor        Full local Rust-Toolchain validation"
    @echo "  rust-tc sonar         Full SonarQube validation + upload"
    @echo "  rust-tc features-deep Deeper Cargo feature combinations"
    @echo "  rust-tc semver PKG    Public API compatibility"
    @echo "  rust-tc mutants       Mutation testing"
    @echo "  rust-tc miri          Undefined-behavior checking"
    @echo "  rust-tc fuzz TARGET   Targeted fuzzing"
    @echo "  rust-tc coverage      Optional local LCOV report"
