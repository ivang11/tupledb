# Tauri grid optimization benchmark

Measured on macOS on 2026-09-03 with release builds and five repetitions per
dataset. Values are medians. The benchmark renders the same table-only fixture
in every build and animates horizontal scrolling for 180 frames.

## Result

| Dataset | Original first paint | Optimized first paint | Change | Original p95 | Optimized p95 | Original RSS peak | Optimized RSS peak | Optimized max |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 300×20 | 893.5 ms | 956.7 ms | +7.1% | 19 ms | 18 ms | 151.0 MiB | 169.1 MiB | 32 ms |
| 300×200 | 1597.6 ms | 870.4 ms | -45.5% | 35 ms | 18 ms | 296.3 MiB | 285.4 MiB | 63 ms |
| 5000×20 | 911.2 ms | 944.1 ms | +3.6% | 18 ms | 18 ms | 165.7 MiB | 177.9 MiB | 46 ms |
| 5000×200 | 1740.5 ms | 924.2 ms | -46.9% | 35 ms | 18 ms | 448.2 MiB | 354.7 MiB | 29 ms |

The optimized p50 was 17 ms in all four datasets. Optimized p99 was 19, 23,
18, and 23 ms respectively. Before bounding the physical scroll width, the
5000×200 case produced an isolated 292–319 ms median maximum, normally around
78–85% of the outward horizontal traversal. The final median maximum is 29 ms.
With a one-second startup settle, 300×200 and 5000×200 measured 23 ms and 22 ms
respectively, showing that their remaining larger samples occur during the
first few startup frames rather than during sustained scrolling.

## What changed

- The production grid paints only the visible viewport into one canvas instead
  of maintaining a DOM cell for each visible row/column combination.
- Hit testing preserves sorting, selection, resize, context menus, foreign-key
  navigation, inline editing, pending inserts, and delete-key behavior.
- A bounded, visually hidden ARIA table mirrors the visible range after scroll
  settles instead of placing the full result in the accessibility tree.
- Rows cross the Tauri IPC boundary as compact arrays aligned with the column
  metadata. The Rust driver and export paths retain named row objects.
- Column-name lookup is cached, and the DOM renderer remains as a test/fallback
  path with support for both named and compact rows.
- Canvas grids wider than 24,000 physical pixels map that bounded native scroll
  range onto the full logical column width. Normal-width grids remain 1:1. This
  avoids a repeatable WebKit compositing stall without dropping any columns;
  a component test covers navigation through the final logical column.

## Outcome

For wide results, the optimization clears the 25–30% threshold: first paint is
45–47% faster and frame p95 is about 49% lower than the original Tauri grid.
Peak RSS in the largest fixture falls by about 21%, and the bounded native
scroll range removes the repeatable long-frame regression found during the
optimization work.

The macOS long-frame regression is resolved in this benchmark. Linux uses an
additional retained-cell limit for very wide query results to contain transfer
cost and webview memory usage.
