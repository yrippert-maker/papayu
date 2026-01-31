//! Преобразует trace из .papa-yu/traces/<trace_id>.json в golden fixture.
//!
//! Использование:
//!   cargo run --bin trace_to_golden -- <trace_id> [output_path]
//!   cargo run --bin trace_to_golden -- <path/to/trace.json> [output_path]

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::Path;

fn schema_hash() -> String {
    let schema_raw = include_str!("../../config/llm_response_schema.json");
    let mut hasher = Sha256::new();
    hasher.update(schema_raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: trace_to_golden <trace_id|path/to/trace.json> [output_path]");
        std::process::exit(1);
    }
    let input = &args[1];
    let output = args.get(2).map(|s| s.as_str());

    let content = if Path::new(input).is_file() {
        fs::read_to_string(input)?
    } else {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
        let trace_path = Path::new(&manifest_dir)
            .join("../.papa-yu/traces")
            .join(format!("{}.json", input));
        fs::read_to_string(&trace_path)
            .map_err(|e| format!("read {}: {}", trace_path.display(), e))?
    };

    let trace: serde_json::Value = serde_json::from_str(&content)?;
    let golden = trace_to_golden_format(&trace)?;
    let out_json = serde_json::to_string_pretty(&golden)?;

    let out_path = match output {
        Some(p) => p.to_string(),
        None => {
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
            let name = trace.get("trace_id").and_then(|v| v.as_str()).unwrap_or("out");
            format!(
                "{}/../docs/golden_traces/v1/{}_golden.json",
                manifest_dir, name
            )
        }
    };
    fs::create_dir_all(Path::new(&out_path).parent().unwrap_or(Path::new(".")))?;
    fs::write(&out_path, out_json)?;
    println!("Written: {}", out_path);
    Ok(())
}

fn trace_to_golden_format(trace: &serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let schema_version = trace
        .get("schema_version")
        .or_else(|| trace.get("config_snapshot").and_then(|c| c.get("schema_version")))
        .cloned()
        .unwrap_or(serde_json::json!(1));
    let schema_hash_val = trace
        .get("schema_hash")
        .or_else(|| trace.get("config_snapshot").and_then(|c| c.get("schema_hash")))
        .cloned()
        .unwrap_or_else(|| serde_json::Value::String(schema_hash()));

    let validated = trace.get("validated_json").cloned();
    let validated_obj = validated
        .as_ref()
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .or_else(|| validated.clone())
        .unwrap_or(serde_json::Value::Null);

    let config = trace.get("config_snapshot").and_then(|c| c.as_object());
    let strict_json = config
        .and_then(|c| c.get("strict_json"))
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty() && matches!(s.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let validation_outcome = if trace.get("event").and_then(|v| v.as_str()) == Some("VALIDATION_FAILED") {
        "err"
    } else {
        "ok"
    };
    let error_code = trace.get("error").and_then(|v| v.as_str()).map(String::from);

    let golden = serde_json::json!({
        "protocol": {
            "schema_version": schema_version,
            "schema_hash": schema_hash_val
        },
        "request": {
            "mode": trace.get("mode").unwrap_or(&serde_json::Value::Null).clone(),
            "input_chars": trace.get("input_chars").unwrap_or(&serde_json::Value::Null).clone(),
            "token_budget": config.and_then(|c| c.get("max_tokens")).unwrap_or(&serde_json::Value::Null).clone(),
            "strict_json": strict_json,
            "provider": trace.get("provider").unwrap_or(&serde_json::Value::Null).clone(),
            "model": trace.get("model").unwrap_or(&serde_json::Value::Null).clone()
        },
        "context": {
            "context_stats": trace.get("context_stats").cloned().unwrap_or(serde_json::Value::Null),
            "cache_stats": trace.get("cache_stats").cloned().unwrap_or(serde_json::Value::Null)
        },
        "result": {
            "validated_json": validated_obj,
            "validation_outcome": validation_outcome,
            "error_code": error_code
        }
    });
    Ok(golden)
}
