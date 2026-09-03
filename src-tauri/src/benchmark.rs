use serde::Serialize;
use std::sync::OnceLock;
use std::time::Instant;

static PROCESS_STARTED: OnceLock<Instant> = OnceLock::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkConfig {
    rows: usize,
    columns: usize,
    settle_ms: u64,
}

pub fn mark_process_started() {
    let _ = PROCESS_STARTED.set(Instant::now());
}

#[tauri::command]
pub fn benchmark_config() -> Option<BenchmarkConfig> {
    if std::env::var_os("TUPLEDB_BENCHMARK_MODE").is_none() {
        return None;
    }

    let rows = std::env::var("TUPLEDB_BENCHMARK_ROWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300)
        .min(5_000);
    let columns = std::env::var("TUPLEDB_BENCHMARK_COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20)
        .clamp(4, 256);
    let settle_ms = std::env::var("TUPLEDB_BENCHMARK_SETTLE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
        .min(5_000);

    Some(BenchmarkConfig {
        rows,
        columns,
        settle_ms,
    })
}

#[tauri::command]
pub fn report_benchmark_metrics(metrics: Vec<(String, f64)>) {
    if std::env::var_os("TUPLEDB_BENCHMARK_MODE").is_none() {
        return;
    }

    let process_ms = PROCESS_STARTED
        .get()
        .map(|started| started.elapsed().as_secs_f64() * 1_000.0)
        .unwrap_or_default();
    let mut fields = vec![format!("process_ms={process_ms:.3}")];
    fields.extend(metrics.into_iter().filter_map(|(name, value)| {
        name.chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
            .then(|| format!("{name}={value:.3}"))
    }));
    eprintln!("TUPLEDB_TAURI_METRIC {}", fields.join(" "));
}
