#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: ntm-activity-fixture-gate --fixture <path> --live <path>\n"
}

fn flag_value(args: &[String], name: &str) -> Result<String, String> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Err(format!("missing {name}"));
    };
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn parse(path: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("{path}: invalid JSON: {error}"))
}

fn field_set(value: &Value) -> BTreeSet<String> {
    fn visit(value: &Value, prefix: &str, fields: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    fields.insert(path.clone());
                    visit(value, &path, fields);
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, prefix, fields);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    let mut fields = BTreeSet::new();
    visit(value, "", &mut fields);
    fields
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }
    let fixture = match flag_value(&args, "--fixture") {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ERROR fixture_gate: {error}");
            eprint!("{}", usage());
            return ExitCode::from(2);
        }
    };
    let live = match flag_value(&args, "--live") {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ERROR fixture_gate: {error}");
            eprint!("{}", usage());
            return ExitCode::from(2);
        }
    };
    let fixture_value = match parse(&fixture) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("ERROR fixture_gate: {error}");
            return ExitCode::from(1);
        }
    };
    let live_value = match parse(&live) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("ERROR fixture_gate: {error}");
            return ExitCode::from(1);
        }
    };
    let fixture_fields = field_set(&fixture_value);
    let live_fields = field_set(&live_value);
    if fixture_fields.is_empty() || live_fields.is_empty() {
        eprintln!(
            "ERROR fixture_gate: zero fields parsed (fixture={}, live={})",
            fixture_fields.len(),
            live_fields.len()
        );
        return ExitCode::from(1);
    }
    let additions: Vec<_> = live_fields.difference(&fixture_fields).collect();
    if !additions.is_empty() {
        eprintln!(
            "RED fixture_gate: live field set gained {} field(s) (fixture={}, live={})",
            additions.len(),
            fixture_fields.len(),
            live_fields.len()
        );
        return ExitCode::from(1);
    }
    println!(
        "PASS fixture_gate: live field set has no additions (fixture_fields={}, live_fields={}, fixture_only={})",
        fixture_fields.len(),
        live_fields.len(),
        fixture_fields.difference(&live_fields).count()
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_set_ignores_array_cardinality_and_scalar_values() {
        let fixture: Value =
            serde_json::from_str(r#"{"agents":[{"detected_patterns":[],"confidence":0.8}]}"#)
                .unwrap();
        let live: Value = serde_json::from_str(
            r#"{"agents":[{"detected_patterns":["braille_spinner"],"confidence":0.95}]}"#,
        )
        .unwrap();
        assert_eq!(field_set(&fixture), field_set(&live));
    }
}
