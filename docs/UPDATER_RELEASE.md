# Release and Updater Guide

This guide explains how TupleDB publishes signed Tauri updater artifacts through
GitHub Releases.

## Overview

The updater flow works like this:

1. The installed app calls Tauri updater `check()`.
2. Tauri downloads:

   ```text
   https://github.com/ivang11/tupledb/releases/latest/download/latest.json
   ```

3. If `latest.json` contains a newer version than the installed app, Tauri
   downloads the platform-specific updater artifact.
4. Tauri verifies the artifact signature using the `pubkey` in
   `src-tauri/tauri.conf.json`.
5. If the signature is valid, Tauri installs the update and restarts the app.

The repository must be public for this GitHub Releases endpoint to work for
normal users.

## First Update Test

An app can only update itself if the installed build already contains the updater
code.

Example test flow:

1. Build and install `0.7.0` with the updater enabled.
2. Publish release `v0.7.0`.
3. Bump the project to `0.7.1`.
4. Publish release `v0.7.1`.
5. Open the installed `0.7.0` app.
6. Run the update check.
7. The app should detect `0.7.1`, download it, verify it, install it, and
   restart.

Older builds that do not include the updater integration cannot update through
this flow.

## Signing Key

`src-tauri/tauri.conf.json` contains the public updater key. Release builds need
the matching private key so Tauri can sign updater artifacts.

For local builds, store the private key outside the repository:

```bash
mkdir -p "$HOME/.config/tupledb"
cp /tmp/tupledb-updater.key "$HOME/.config/tupledb/updater.key"
```

Then build signed updater artifacts with:

```bash
npm run release:build
```

By default, `release:build` reads:

```text
~/.config/tupledb/updater.key
```

If the key lives elsewhere, set:

```bash
TAURI_SIGNING_PRIVATE_KEY_PATH=/path/to/updater.key npm run release:build
```

The script reads the file and passes its contents to Tauri through
`TAURI_SIGNING_PRIVATE_KEY`.

Never commit the private key and never upload it to a public release. In GitHub
Actions, store it as a repository secret.

## Versioning

Update all project version files with:

```bash
npm run version:set -- 0.7.1
```

This updates:

- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

It also runs `cargo check` so `src-tauri/Cargo.lock` stays in sync.

Check that old versions are gone:

```bash
rg "0.7.0|0.7.1" package.json package-lock.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
```

## Local Signed Builds

Run:

```bash
npm run release:build
```

The default build target is `tauri:build:ci`. GitHub Actions overrides this with
platform-specific scripts:

- `tauri:build:linux`
- `tauri:build:macos`

If you only need a local app build without updater artifacts, use:

```bash
npm run build:app
```

Common signing error:

```text
A public key has been found, but no private key.
```

This means `TAURI_SIGNING_PRIVATE_KEY_PATH` or `TAURI_SIGNING_PRIVATE_KEY` is
missing.

Linux packaging error:

```text
failed to run linuxdeploy
```

This means Rust compilation already finished, but AppImage packaging failed.
Check the local Linux/Tauri packaging dependencies before publishing release
assets.

## GitHub Actions Release Flow

The workflow lives in:

```text
.github/workflows/release.yml
```

It runs when a `v*` tag is pushed.

When triggered, GitHub Actions:

1. Checks that the tag version matches `package.json`.
2. Builds a signed Linux x86_64 AppImage.
3. Builds signed macOS updater artifacts and DMG installers for Intel and Apple
   Silicon.
4. Downloads all build artifacts into the publish job.
5. Generates one `latest.json` manifest with:

   ```text
   linux-x86_64
   darwin-x86_64
   darwin-aarch64
   ```

6. Publishes or updates one GitHub Release.

## Required GitHub Secrets

In GitHub, open **Settings > Secrets and variables > Actions** and create:

```text
TAURI_SIGNING_PRIVATE_KEY
```

It must contain the full private updater key:

```bash
cat "$HOME/.config/tupledb/updater.key"
```

If the key has a password, also create:

```text
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

If the key has no password, this secret is not required.

## Release Assets

The release script uses human-readable names for first-time downloads:

```text
TupleDB_0.7.0_Linux_x86_64.AppImage
TupleDB_0.7.0_macOS_Apple_Silicon.dmg
TupleDB_0.7.0_macOS_Intel.dmg
```

The release also includes updater-only artifacts:

```text
TupleDB_0.7.0_macOS_Apple_Silicon_updater.app.tar.gz
TupleDB_0.7.0_macOS_Intel_updater.app.tar.gz
```

Those `.app.tar.gz` files are for the built-in updater, not for normal manual
installation.

## latest.json

`latest.json` is the manifest read by the updater. The `signature` values must be
the contents of the `.sig` files, not paths to `.sig` files.

Example:

```json
{
  "version": "0.7.0",
  "notes": "TupleDB 0.7.0",
  "pub_date": "2026-07-21T00:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "SIGNATURE_FILE_CONTENTS",
      "url": "https://github.com/ivang11/tupledb/releases/download/v0.7.0/TupleDB_0.7.0_Linux_x86_64.AppImage"
    },
    "darwin-x86_64": {
      "signature": "SIGNATURE_FILE_CONTENTS",
      "url": "https://github.com/ivang11/tupledb/releases/download/v0.7.0/TupleDB_0.7.0_macOS_Intel_updater.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "SIGNATURE_FILE_CONTENTS",
      "url": "https://github.com/ivang11/tupledb/releases/download/v0.7.0/TupleDB_0.7.0_macOS_Apple_Silicon_updater.app.tar.gz"
    }
  }
}
```

Tauri validates the whole manifest before applying the version comparison, so
every platform entry present in the file must have a valid URL and signature.

## Manual Publishing

Manual publishing is normally not needed because GitHub Actions does it. If you
need to test the publishing script locally, run:

```bash
npm run release:publish -- --dry-run
```

To publish from existing local artifacts:

```bash
npm run release:publish
```

Useful options:

```bash
npm run release:publish -- --version 0.7.0 --tag v0.7.0 --notes "TupleDB 0.7.0"
npm run release:publish -- --repo ivang11/tupledb
```

The script requires GitHub CLI:

```bash
gh auth login
```

## Recommended Release Command Sequence

For a new version:

```bash
npm run version:set -- 0.7.1
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "Release 0.7.1"
git tag v0.7.1
git push origin main
git push origin v0.7.1
```

Avoid `git push origin main --tags` for releases. If multiple local tags exist,
GitHub may receive more tags than intended.

## Release Checklist

- The version is updated in `package.json`, `package-lock.json`,
  `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- The build used the private key that matches the public updater key.
- Every updater artifact has a matching `.sig`.
- `latest.json` contains a newer version than the installed app.
- `latest.json` contains signature contents, not signature file paths.
- Every URL in `latest.json` points to a real GitHub Release asset.
- The release is published, not only saved as a draft.
