#![forbid(unsafe_code)]

//! Pure decision logic for the fleet composite.
//!
//! The headline is intentionally geometric rather than arithmetic.  An arithmetic mean can
//! hide a dead dispatch dimension behind healthy commits; the geometric mean makes any factor at
//! baseline zero the whole headline.  Input parsing and malformed-data handling are also kept
//! here so every caller gets the same fail-closed behavior.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

pub const SCHEMA: &str = "zs.fleet-composite.v1";
pub const MEAN_KIND: &str = "geometric";

/// A named baseline-to-optimum scoring dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FactorSpec {
    pub name: &'static str,
    pub baseline: f64,
    pub optimum: f64,
}

/// The four factors are ordered as in the Python oracle.  Output maps are sorted independently
/// for deterministic machine-readable output.
pub const FACTORS: [FactorSpec; 4] = [
    FactorSpec {
        name: "commits_1h",
        baseline: 0.0,
        optimum: 6.0,
    },
    FactorSpec {
        name: "omp_busy",
        baseline: 0.0,
        optimum: 3.0,
    },
    FactorSpec {
        name: "ledger_fresh",
        baseline: 0.0,
        optimum: 1.0,
    },
    FactorSpec {
        name: "beads_closed_1h",
        baseline: 0.0,
        optimum: 3.0,
    },
];

pub fn factors() -> &'static [FactorSpec; 4] {
    &FACTORS
}

/// Structured parse failures prevent malformed input from becoming an apparently healthy score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputError {
    Empty,
    MalformedJson,
    RootNotObject,
    FactorNotNumber { name: String },
    NonFinite { name: String },
}

impl InputError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty_input",
            Self::MalformedJson => "malformed_json",
            Self::RootNotObject => "root_not_object",
            Self::FactorNotNumber { .. } => "factor_not_number",
            Self::NonFinite { .. } => "factor_non_finite",
        }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("input is empty"),
            Self::MalformedJson => f.write_str("input is not valid JSON"),
            Self::RootNotObject => f.write_str("input root must be a JSON object"),
            Self::FactorNotNumber { name } => write!(f, "factor {name:?} must be a JSON number"),
            Self::NonFinite { name } => write!(f, "factor {name:?} must be finite"),
        }
    }
}

impl std::error::Error for InputError {}

/// Parse a JSON object containing numeric factor values.
///
/// Unknown numeric keys are preserved for compatibility with the Python `compute` function, but
/// malformed values refuse the entire payload instead of silently dropping one factor.
pub fn parse_raw(input: &str) -> Result<BTreeMap<String, f64>, InputError> {
    if input.trim().is_empty() {
        return Err(InputError::Empty);
    }
    let value: Value = serde_json::from_str(input).map_err(|_| InputError::MalformedJson)?;
    let object = value.as_object().ok_or(InputError::RootNotObject)?;
    let mut raw = BTreeMap::new();
    for (name, value) in object {
        let score = value
            .as_f64()
            .ok_or_else(|| InputError::FactorNotNumber { name: name.clone() })?;
        if !score.is_finite() {
            return Err(InputError::NonFinite { name: name.clone() });
        }
        raw.insert(name.clone(), score);
    }
    Ok(raw)
}

/// Fraction of the baseline-to-optimum gap closed by `score`.
///
/// A zero-width factor earns no credit rather than dividing by zero.  Callers still clamp the
/// result for the geometric product; retaining the negative branch makes regressions visible in
/// `closed_pct` while ensuring they cannot improve the headline.
pub fn closed_fraction(score: f64, baseline: f64, optimum: f64) -> f64 {
    let denom = optimum - baseline;
    if denom == 0.0 || !denom.is_finite() || !score.is_finite() {
        return 0.0;
    }
    (score - baseline) / denom
}

fn normalized_epsilon(eps: f64) -> f64 {
    if eps.is_finite() {
        eps.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Geometric mean of closed fractions, clamped to `[0, 1]` by default.
pub fn geometric_headline(closed: &BTreeMap<String, f64>) -> f64 {
    geometric_headline_with_epsilon(closed, 0.0)
}

/// Geometric mean with the Python oracle's configurable lower clamp.
///
/// Invalid epsilon and non-finite factors are treated as zero credit, not as a NaN headline that
/// downstream JSON encoders or operators could misread.
pub fn geometric_headline_with_epsilon(closed: &BTreeMap<String, f64>, eps: f64) -> f64 {
    if closed.is_empty() {
        return 0.0;
    }
    let eps = normalized_epsilon(eps);
    let mut product = 1.0;
    for value in closed.values() {
        if !value.is_finite() {
            return 0.0;
        }
        let clamped = value.clamp(eps, 1.0);
        product *= clamped;
    }
    product.powf(1.0 / closed.len() as f64)
}

/// Arithmetic mean used only as a mutation oracle: it demonstrates the defect the geometric
/// headline prevents and is never used by [`compute`].
pub fn arithmetic_headline(closed: &BTreeMap<String, f64>) -> f64 {
    if closed.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for value in closed.values() {
        if !value.is_finite() {
            return 0.0;
        }
        total += value.clamp(0.0, 1.0);
    }
    total / closed.len() as f64
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompositeReport {
    pub schema: &'static str,
    pub headline_pct: f64,
    pub raw: BTreeMap<String, f64>,
    pub closed_pct: BTreeMap<String, f64>,
    pub dead_factors: Vec<String>,
    pub mean_kind: &'static str,
    pub verdict: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_error: Option<InputError>,
}

impl CompositeReport {
    fn refused(error: InputError) -> Self {
        let mut dead_factors: Vec<String> = FACTORS.iter().map(|factor| factor.name.to_owned()).collect();
        dead_factors.sort();
        Self {
            schema: SCHEMA,
            headline_pct: 0.0,
            raw: BTreeMap::new(),
            closed_pct: BTreeMap::new(),
            dead_factors,
            mean_kind: MEAN_KIND,
            verdict: "DEAD",
            input_error: Some(error),
        }
    }
}

/// Compute the report using the hard-zero geometric default.
pub fn compute(raw: &BTreeMap<String, f64>) -> CompositeReport {
    compute_with_epsilon(raw, 0.0)
}

/// Compute a report with a caller-selected geometric lower clamp.
pub fn compute_with_epsilon(raw: &BTreeMap<String, f64>, eps: f64) -> CompositeReport {
    if let Some((name, _)) = raw.iter().find(|(_, value)| !value.is_finite()) {
        return CompositeReport::refused(InputError::NonFinite { name: name.clone() });
    }

    let mut closed = BTreeMap::new();
    for factor in FACTORS {
        if let Some(score) = raw.get(factor.name) {
            closed.insert(
                factor.name.to_owned(),
                closed_fraction(*score, factor.baseline, factor.optimum),
            );
        }
    }

    let headline = geometric_headline_with_epsilon(&closed, eps);
    let dead_factors = closed
        .iter()
        .filter(|(_, value)| **value <= 0.0)
        .map(|(name, _)| name.clone())
        .collect();
    let verdict = if headline <= 0.0 {
        "DEAD"
    } else if headline < 0.5 {
        "WEAK"
    } else {
        "OK"
    };

    CompositeReport {
        schema: SCHEMA,
        headline_pct: round_two(headline * 100.0),
        raw: raw.clone(),
        closed_pct: closed
            .into_iter()
            .map(|(name, value)| (name, round_two(value * 100.0)))
            .collect(),
        dead_factors,
        mean_kind: MEAN_KIND,
        verdict,
        input_error: None,
    }
}

/// Parse and compute a JSON payload, refusing malformed or empty input as a DEAD report.
pub fn compute_json(input: &str) -> CompositeReport {
    compute_json_with_epsilon(input, 0.0)
}

pub fn compute_json_with_epsilon(input: &str, eps: f64) -> CompositeReport {
    match parse_raw(input) {
        Ok(raw) => compute_with_epsilon(&raw, eps),
        Err(error) => CompositeReport::refused(error),
    }
}

fn round_two(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    // Python's round(x, 2) uses ties-to-even.  This keeps JSON reports stable at exact half-cent
    // boundaries rather than using Rust's ties-away-from-zero `f64::round`.
    let scaled = value * 100.0;
    let lower = scaled.floor();
    let fraction = scaled - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 {
        lower + 1.0
    } else if (lower % 2.0).abs() < f64::EPSILON {
        lower
    } else {
        lower + 1.0
    };
    rounded / 100.0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelftestCheck {
    pub label: &'static str,
    pub passed: bool,
    pub got: String,
    pub want: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelftestReport {
    pub checked: usize,
    pub failures: Vec<&'static str>,
    pub checks: Vec<SelftestCheck>,
}

fn record<T: fmt::Debug + PartialEq>(checks: &mut Vec<SelftestCheck>, label: &'static str, got: T, want: T) {
    let passed = got == want;
    checks.push(SelftestCheck {
        label,
        passed,
        got: format!("{got:?}"),
        want: format!("{want:?}"),
    });
}

/// Run all eleven Python-oracle assertions, including the arithmetic mutation control.
pub fn run_selftest() -> SelftestReport {
    let mut checks = Vec::new();
    let live = BTreeMap::from([
        ("commits_1h".to_owned(), 7.0),
        ("omp_busy".to_owned(), 0.0),
        ("ledger_fresh".to_owned(), 0.0),
        ("beads_closed_1h".to_owned(), 0.0),
    ]);
    let report = compute(&live);
    record(&mut checks, "MEASURED row scores ZERO", report.headline_pct, 0.0);
    record(
        &mut checks,
        "MEASURED row names dead factors",
        report.dead_factors,
        vec![
            "beads_closed_1h".to_owned(),
            "ledger_fresh".to_owned(),
            "omp_busy".to_owned(),
        ],
    );
    record(&mut checks, "MEASURED row verdict is DEAD", report.verdict, "DEAD");

    let healthy = BTreeMap::from([
        ("commits_1h".to_owned(), 6.0),
        ("omp_busy".to_owned(), 3.0),
        ("ledger_fresh".to_owned(), 1.0),
        ("beads_closed_1h".to_owned(), 3.0),
    ]);
    record(
        &mut checks,
        "healthy fleet scores 100",
        compute(&healthy).headline_pct,
        100.0,
    );

    let spiky = BTreeMap::from([
        ("commits_1h".to_owned(), 6.0),
        ("omp_busy".to_owned(), 3.0),
        ("ledger_fresh".to_owned(), 0.0),
        ("beads_closed_1h".to_owned(), 0.0),
    ]);
    let balanced = BTreeMap::from([
        ("commits_1h".to_owned(), 3.0),
        ("omp_busy".to_owned(), 1.5),
        ("ledger_fresh".to_owned(), 0.5),
        ("beads_closed_1h".to_owned(), 1.5),
    ]);
    let spiky_score = compute(&spiky).headline_pct;
    let balanced_score = compute(&balanced).headline_pct;
    record(&mut checks, "spiky scores ZERO", spiky_score, 0.0);
    record(&mut checks, "balanced scores 50", balanced_score, 50.0);
    record(
        &mut checks,
        "balanced beats spiky at equal total",
        balanced_score > spiky_score,
        true,
    );

    let regression = BTreeMap::from([
        ("commits_1h".to_owned(), -1.0),
        ("omp_busy".to_owned(), 3.0),
        ("ledger_fresh".to_owned(), 1.0),
        ("beads_closed_1h".to_owned(), 3.0),
    ]);
    record(
        &mut checks,
        "negative factor clamps to zero",
        compute(&regression).headline_pct,
        0.0,
    );

    let overshoot = BTreeMap::from([
        ("commits_1h".to_owned(), 600.0),
        ("omp_busy".to_owned(), 1.5),
        ("ledger_fresh".to_owned(), 1.0),
        ("beads_closed_1h".to_owned(), 1.5),
    ]);
    record(
        &mut checks,
        "overshoot caps at optimum",
        compute(&overshoot).headline_pct,
        70.71,
    );

    let mut mutation_closed = BTreeMap::new();
    for (name, score) in &live {
        if let Some(factor) = FACTORS.iter().find(|factor| factor.name == name) {
            mutation_closed.insert(
                name.clone(),
                closed_fraction(*score, factor.baseline, factor.optimum),
            );
        }
    }
    let arithmetic = round_two(arithmetic_headline(&mutation_closed) * 100.0);
    record(&mut checks, "arithmetic mutation is nonzero", arithmetic > 0.0, true);
    record(&mut checks, "arithmetic mutation reports 25", arithmetic, 25.0);

    let failures = checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.label)
        .collect();
    SelftestReport {
        checked: checks.len(),
        failures,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_fraction_rejects_zero_width_and_preserves_regression() {
        assert_eq!(closed_fraction(2.0, 1.0, 1.0), 0.0);
        assert_eq!(closed_fraction(-1.0, 0.0, 6.0), -1.0 / 6.0);
    }

    #[test]
    fn geometric_empty_input_is_hard_zero() {
        assert_eq!(geometric_headline(&BTreeMap::new()), 0.0);
    }

    #[test]
    fn malformed_json_is_structured_and_fail_closed() {
        let report = compute_json("not-json");
        assert_eq!(report.verdict, "DEAD");
        assert_eq!(report.headline_pct, 0.0);
        assert_eq!(report.input_error.as_ref().map(InputError::code), Some("malformed_json"));
    }

    #[test]
    fn mutation_control_cannot_be_satisfied_by_arithmetic_compute() {
        let report = run_selftest();
        assert_eq!(report.checked, 11);
        assert!(report.failures.is_empty(), "{report:?}");
        assert_eq!(compute(&BTreeMap::from([(String::from("commits_1h"), 7.0)])).mean_kind, "geometric");
    }
}
