# Tauri grid optimization experiment

Date: 1 September 2026

## Outcome

The final canvas grid clears the performance threshold on macOS. The 5,000×200 benchmark
improves first paint from 1,740.5 ms to 924.2 ms, frame p95 from 35 ms to 18 ms, and peak
RSS from 448.2 MiB to 354.7 MiB. Bounding WebKit's physical horizontal scroll surface
also reduces the median maximum frame from the intermediate 292–319 ms regression to
29 ms. See [the complete comparison](performance/tauri-grid-optimization.md).

## Changes under test

- Replaced the visible browser table with a viewport-sized canvas renderer and a bounded,
  visually hidden ARIA table.
- Added horizontal and vertical viewport virtualization while preserving the native
  scrollbar.
- Mapped grids wider than 24,000 physical pixels onto their complete logical width to
  avoid WebKit's wide-layer compositing stall.
- Changed Tauri's table-data IPC response to compact row arrays while keeping internal
  Rust and export paths as named objects.
- Preserved column resize, sorting, row selection, inline editing, pending inserts, foreign
  key navigation, keyboard deletion, and row context menus.
- Added an opt-in release benchmark and a component test for horizontal virtualization.

## macOS five-run median

| Dataset | First paint | RSS peak | Frame p50 | Frame p95 | Frame p99 | Frame max |
|---:|---:|---:|---:|---:|---:|---:|
| 300×20 | 956.7 ms | 169.1 MiB | 17.00 ms | 18.00 ms | 19.00 ms | 32.00 ms |
| 300×200 | 870.4 ms | 285.4 MiB | 17.00 ms | 18.00 ms | 23.00 ms | 63.00 ms |
| 5,000×20 | 944.1 ms | 177.9 MiB | 17.00 ms | 18.00 ms | 18.00 ms | 46.00 ms |
| 5,000×200 | 924.2 ms | 354.7 MiB | 17.00 ms | 18.00 ms | 23.00 ms | 29.00 ms |

## Reproduction on macOS or Linux

Install Node.js 24 and the stable Rust toolchain first. On Ubuntu 22.04, install the same
native dependencies used by the release workflow:

```bash
sudo apt-get update
sudo apt-get install -y build-essential curl wget file libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev libxdo-dev librsvg2-dev patchelf libssl-dev libfuse2
```

Then run:

```bash
npm ci
npm run test:unit
npm run test:component
cargo test --manifest-path src-tauri/Cargo.toml
npm run benchmark:tauri-grid -- --repetitions=5
```

Raw JSON and Markdown output are written under the ignored
`src-tauri/target/benchmarks/` directory. Record the OS, desktop session (X11 or Wayland
on Linux), CPU, and display scaling when sharing results.
