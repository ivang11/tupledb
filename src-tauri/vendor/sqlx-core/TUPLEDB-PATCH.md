# SQLx 0.8.6 TLS compatibility backport

This directory contains the unmodified `src/`, normalized `Cargo.toml`, and
MIT/Apache-2.0 licenses from the published `sqlx-core` 0.8.6 crate, except for
the upstream changes and local lifetime annotations below. No registry cache or
build outputs are vendored.

Source: <https://crates.io/crates/sqlx-core/0.8.6>

Upstream fix: <https://github.com/launchbadge/sqlx/pull/3861>

Commit: `2970559e256ff11194cdc7c7aa18eb9d64fce494`

1. Accept both `NotValidForName` and `NotValidForNameContext` in SQLx's existing
   `NoHostnameTlsVerifier` (used by PostgreSQL `verify_ca`).
2. Set the minimum rustls version to 0.23.24, which exposes the contextual error.

Local compiler cleanup: use the existing `'q` lifetime explicitly for statement
arguments in `query_statement`, `query_statement_as` and `query_statement_scalar`.
This is equivalent to the previous elided lifetime and removes
`mismatched_lifetime_syntaxes` warnings on newer Rust compilers.

Certificate-chain, expiry and handshake-signature checks are unchanged. Only
hostname mismatch is ignored in `verify_ca`; `verify_full` keeps its standard
verifier. Invalid certificates must never be retried using `require`.

Why local: there is no 0.8.7 release with this fix. SQLx 0.9 contains it but also
changes dynamic-query and argument APIs across both database adapters. Keeping
this narrow upstream backport avoids coupling that migration to a TLS fix and
does not pin rustls to an old release.

## Maintenance

Run `node scripts/test-postgresql-transport.mjs` from the repository root to test
real TLS/SSH handshakes and database transfers with temporary certificates.
Also run the PostgreSQL and MySQL integration suites: the core crate is shared.

Remove this directory and `[patch.crates-io]` when upgrading to a SQLx release
that includes #3861. Confirm both hostname-mismatch cases succeed in `verify_ca`,
while wrong CA, invalid/expired certificates and `verify_full` hostname mismatch
still fail. Keep the transport regression tests after removing the backport.
