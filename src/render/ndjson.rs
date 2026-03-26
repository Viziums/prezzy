// Epoch math intentionally uses lossy numeric casts.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]

use std::io::Write;

use anyhow::Result;
use crossterm::style::Stylize;
use serde_json::Value;

use super::{RenderContext, Renderer};
use crate::theme::LogColors;

/// Renders NDJSON as a structured log view.
///
/// Extracts well-known fields (timestamp, level, message) and renders
/// remaining fields as key=value pairs.
pub struct NdjsonRenderer;

impl Renderer for NdjsonRenderer {
    #[allow(clippy::single_match_else, clippy::branches_sharing_code)]
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let obj: Value = match serde_json::from_str(trimmed) {
            Ok(v @ Value::Object(_)) => v,
            // Not a JSON object -- pass through raw.
            _ => {
                write!(writer, "{line}")?;
                return Ok(());
            }
        };

        let map = obj.as_object().unwrap();
        let colors = &ctx.theme.log;

        // Extract well-known fields.
        let timestamp = extract_string(
            map,
            &["ts", "time", "timestamp", "@timestamp", "t", "datetime"],
        );
        let level = extract_string(map, &["level", "lvl", "severity", "log.level"]);

        // Apply level filter.
        if let (Some(filter), Some(lvl)) = (ctx.level_filter, &level) {
            if !filter.passes(lvl) {
                return Ok(());
            }
        }
        let message = extract_string(map, &["msg", "message", "text", "body"]);

        // Render: timestamp level message extra_fields
        if ctx.terminal.color_enabled {
            if let Some(ts) = &timestamp {
                write!(writer, "{} ", ts.as_str().with(colors.timestamp))?;
            }
            if let Some(lvl) = &level {
                write_level_colored(writer, lvl, colors)?;
                write!(writer, " ")?;
            }
            if let Some(msg) = &message {
                write!(writer, "{msg}")?;
            }
        } else {
            if let Some(ts) = &timestamp {
                write!(writer, "{ts} ")?;
            }
            if let Some(lvl) = &level {
                write!(writer, "{:5} ", lvl.to_uppercase())?;
            }
            if let Some(msg) = &message {
                write!(writer, "{msg}")?;
            }
        }

        // Remaining fields as key=value.
        let skip_keys: &[&str] = &[
            "ts",
            "time",
            "timestamp",
            "@timestamp",
            "t",
            "datetime",
            "level",
            "lvl",
            "severity",
            "log.level",
            "msg",
            "message",
            "text",
            "body",
        ];

        let mut has_extra = false;
        for (key, val) in map {
            if skip_keys.contains(&key.as_str()) {
                continue;
            }
            if has_extra {
                write!(writer, " ")?;
            } else {
                write!(writer, "  ")?;
                has_extra = true;
            }

            let val_str = format_value(val);
            if ctx.terminal.color_enabled {
                write!(writer, "{}={}", key.as_str().with(colors.context), val_str)?;
            } else {
                write!(writer, "{key}={val_str}")?;
            }
        }

        Ok(())
    }
}

/// Write the log level with appropriate color and fixed-width padding.
fn write_level_colored(writer: &mut dyn Write, level: &str, colors: &LogColors) -> Result<()> {
    let upper = level.to_uppercase();
    let color = match upper.as_str() {
        "ERROR" | "ERR" | "FATAL" | "CRITICAL" | "CRIT" => colors.error,
        "WARN" | "WARNING" => colors.warn,
        "DEBUG" | "DBG" => colors.debug,
        "TRACE" | "TRC" | "VERBOSE" => colors.trace,
        _ => colors.info, // INFO and unrecognized levels
    };
    write!(writer, "{:5}", upper.with(color))?;
    Ok(())
}

/// Extract the first matching field from a JSON object.
fn extract_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(val) = map.get(*key) {
            return Some(match val {
                Value::String(s) => s.clone(),
                Value::Number(n) => {
                    // Numeric timestamps (epoch seconds/millis).
                    if let Some(f) = n.as_f64() {
                        if f > 1_000_000_000.0 && f < 2_000_000_000.0 {
                            // Epoch seconds -- format as ISO 8601.
                            return Some(format_epoch(f));
                        }
                        if f > 1_000_000_000_000.0 {
                            // Epoch millis.
                            return Some(format_epoch(f / 1000.0));
                        }
                    }
                    n.to_string()
                }
                _ => val.to_string(),
            });
        }
    }
    None
}

/// Format epoch seconds as a human-readable timestamp.
fn format_epoch(epoch: f64) -> String {
    let secs = epoch as i64;
    let millis = ((epoch - secs as f64) * 1000.0) as u32;

    // Manual UTC formatting (avoids chrono dependency).
    let (year, month, day, hour, min, sec) = epoch_to_datetime(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{millis:03}Z")
}

/// Convert epoch seconds to (year, month, day, hour, minute, second).
const fn epoch_to_datetime(epoch: i64) -> (i64, u32, u32, u32, u32, u32) {
    let secs_per_day: i64 = 86400;
    let days = epoch / secs_per_day;
    let time_of_day = (epoch % secs_per_day) as u32;

    let hour = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;
    let sec = time_of_day % 60;

    // Days since 1970-01-01 to calendar date.
    let (year, month, day) = days_to_date(days);
    (year, month, day, hour, min, sec)
}

/// Civil date from days since Unix epoch. Algorithm from Howard Hinnant.
const fn days_to_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a JSON value for display as a log extra field.
fn format_value(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        // Arrays/objects: compact JSON.
        _ => val.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_epoch_produces_iso() {
        let ts = format_epoch(1_700_000_000.123);
        assert!(ts.starts_with("2023-11-14T22:13:20"));
    }
}
