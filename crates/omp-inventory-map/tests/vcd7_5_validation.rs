#![forbid(unsafe_code)]

use serde_json::Value;
use sha2::{Digest, Sha256};

const TRANSCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/vcd7.5-br-bv-ntm-transcript.json"
));
const TRANSCRIPT_SHA256: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/vcd7.5-br-bv-ntm-transcript.json.sha256"
));
const EXPECTED_BR: &str = "br 0.4.1";
const EXPECTED_BV: &str = "bv v0.20.0";
const EXPECTED_NTM: &str = "ntm version v1.31.0-4-ge4718530";

fn transcript_value() -> Value {
    serde_json::from_str(TRANSCRIPT).expect("pinned validation transcript must be JSON")
}

fn validate_transcript(value: &Value) -> Result<(), String> {
    if value.get("schema_version").and_then(Value::as_str)
        != Some("omp-orchestrator/vcd7.5-validation/v1")
    {
        return Err("transcript schema version mismatch".to_owned());
    }
    let versions = value
        .get("versions")
        .and_then(Value::as_object)
        .ok_or_else(|| "transcript versions object missing".to_owned())?;
    for (name, expected) in [
        ("br", EXPECTED_BR),
        ("bv", EXPECTED_BV),
        ("ntm", EXPECTED_NTM),
    ] {
        if versions.get(name).and_then(Value::as_str) != Some(expected) {
            return Err(format!("{name} version mismatch"));
        }
    }

    let rows = value
        .get("validate_rows")
        .and_then(Value::as_array)
        .ok_or_else(|| "VALIDATE row set missing".to_owned())?;
    if rows.len() != 8 {
        return Err(format!(
            "VALIDATE row set has {} rows, expected 8",
            rows.len()
        ));
    }
    let expected_rows = [
        ("br:close", "Usage: br close"),
        ("br:create", "Usage: br create"),
        ("br:init", "Usage: br init"),
        ("br:list", "Usage: br list"),
        ("br:schema", "Emit JSON Schemas"),
        ("br:sync", "Usage: br sync"),
        ("br:update", "Usage: br update"),
        ("bv:exit-codes", ""),
    ];
    for (id, marker) in expected_rows {
        let matches: Vec<&Value> = rows
            .iter()
            .filter(|row| row.get("id").and_then(Value::as_str) == Some(id))
            .collect();
        if matches.len() != 1 {
            return Err(format!("{id} appears {} times", matches.len()));
        }
        let row = matches[0];
        if row.get("observed_exit").and_then(Value::as_i64) != Some(0)
            || row.get("expected_exit").and_then(Value::as_i64) != Some(0)
        {
            return Err(format!("{id} is not pinned to exit 0"));
        }
        let stdout = row.get("stdout").and_then(Value::as_str).unwrap_or("");
        if stdout.is_empty() {
            return Err(format!("{id} has an empty stdout transcript"));
        }
        if !marker.is_empty() && !stdout.contains(marker) {
            return Err(format!("{id} stdout lacks shape marker {marker}"));
        }
        if id == "bv:exit-codes" {
            let parsed: Value = serde_json::from_str(stdout)
                .map_err(|error| format!("{id} JSON shape invalid: {error}"))?;
            for field in row
                .get("json_required")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{id} required-field list missing"))?
            {
                let field = field
                    .as_str()
                    .ok_or_else(|| format!("{id} non-string required field"))?;
                if parsed.get(field).is_none() {
                    return Err(format!("{id} JSON omits {field}"));
                }
            }
        }
    }

    let json_probes = value
        .get("json_probes")
        .and_then(Value::as_array)
        .ok_or_else(|| "JSON probe set missing".to_owned())?;
    for probe in json_probes {
        if probe.get("observed_exit").and_then(Value::as_i64) != Some(0) {
            return Err(format!("{} JSON probe is not exit 0", probe["id"]));
        }
        let output = probe
            .get("stdout")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} JSON stdout missing", probe["id"]))?;
        let parsed: Value = serde_json::from_str(output)
            .map_err(|error| format!("{} JSON shape invalid: {error}", probe["id"]))?;
        for field in probe
            .get("json_required")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{} required-field list missing", probe["id"]))?
        {
            let field = field
                .as_str()
                .ok_or_else(|| "non-string required field".to_owned())?;
            if parsed.get(field).is_none() {
                return Err(format!("{} JSON omits {field}", probe["id"]));
            }
        }
        if probe.get("id").and_then(Value::as_str) == Some("br:list-json") {
            let issues = parsed
                .get("issues")
                .and_then(Value::as_array)
                .ok_or_else(|| "br:list-json JSON omits issues array".to_owned())?;
            if issues.is_empty() {
                return Err("br:list-json positive control returned no issues".to_owned());
            }
        }
    }

    let omissions = value
        .get("template_omissions")
        .and_then(Value::as_array)
        .ok_or_else(|| "template omission set missing".to_owned())?;
    if omissions.len() != 2 {
        return Err(format!(
            "template omission set has {} rows, expected 2",
            omissions.len()
        ));
    }
    for omission in omissions {
        let omitted = omission
            .get("omitted")
            .and_then(Value::as_str)
            .ok_or_else(|| "omitted variable missing".to_owned())?;
        if omission.get("observed_exit").and_then(Value::as_i64) != Some(1)
            || !omission
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains(&format!("missing required variable: {omitted}"))
        {
            return Err(format!("ntm omission does not name {omitted}"));
        }
    }

    let positive = value
        .get("positive_control")
        .ok_or_else(|| "positive control missing".to_owned())?;
    if positive
        .get("br_list_json_nonempty")
        .and_then(Value::as_bool)
        != Some(true)
        || positive.get("bv_next_json_object").and_then(Value::as_bool) != Some(true)
        || positive
            .get("empty_scan_set_verdict")
            .and_then(Value::as_str)
            != Some("ERROR")
    {
        return Err("positive control or empty-scan error contract missing".to_owned());
    }
    Ok(())
}

#[test]
fn validate_rows_match_pinned_installed_versions_and_shapes() {
    validate_transcript(&transcript_value()).expect("pinned VALIDATE transcript must hold");
}

#[test]
fn version_or_shape_drift_is_red() {
    let mut version_bump = transcript_value();
    version_bump["versions"]["br"] = Value::String("br 0.4.2".to_owned());
    let version_error = validate_transcript(&version_bump).expect_err("version drift must be RED");
    assert!(
        version_error.contains("br version mismatch"),
        "{version_error}"
    );

    let mut shape_bump = transcript_value();
    shape_bump["json_probes"][0]["stdout"] =
        Value::String(r#"{"total":1,"limit":1,"offset":0,"has_more":false}"#.to_owned());
    let shape_error = validate_transcript(&shape_bump).expect_err("JSON shape drift must be RED");
    assert!(shape_error.contains("JSON omits issues"), "{shape_error}");
}

#[test]
fn template_omissions_are_typed_and_positive_controls_are_nonvacuous() {
    let value = transcript_value();
    validate_transcript(&value).expect("omission and positive-control contracts");
    let omissions = value["template_omissions"].as_array().expect("omissions");
    assert!(omissions.iter().all(|row| row["observed_exit"] == 1));
    assert!(omissions.iter().any(|row| row["omitted"] == "objective"));
    assert!(omissions.iter().any(|row| row["omitted"] == "target"));
}

#[test]
fn empty_validation_set_is_an_error() {
    let mut value = transcript_value();
    value["validate_rows"] = Value::Array(Vec::new());
    let error = validate_transcript(&value).expect_err("empty validation set must be RED");
    assert!(error.contains("expected 8"), "{error}");
}

#[test]
fn transcript_sha256_matches_sidecar() {
    let digest = Sha256::digest(TRANSCRIPT.as_bytes());
    let actual: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let expected = TRANSCRIPT_SHA256
        .split_whitespace()
        .next()
        .expect("sha sidecar must contain a digest");
    assert_eq!(actual, expected);
}
