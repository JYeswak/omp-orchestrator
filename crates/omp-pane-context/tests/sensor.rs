#![forbid(unsafe_code)]

//! Contract tests for the session-file context sensor. Every fixture mirrors a shape measured
//! on omp 18.3.1 (2026-09-25): four-line tty-keyed breadcrumbs, `contextSnapshot.promptTokens`
//! on assistant entries, `compaction.tokensAfter`, `model_change.model = provider/id`.

use omp_pane_context::{
    find_omp, is_omp_argv, last_usage_record, max_percent_text, parse_breadcrumb, parse_etime,
    parse_ps, read_pane, terminal_id, OmpProcess, PaneContext, PaneObservation, Profile,
    ReadingSource, TailScan, UnknownReason, WindowCatalog, UNKNOWN,
};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct FakeCatalog(HashMap<(String, String), u64>);

impl FakeCatalog {
    fn standard() -> Self {
        let mut map = HashMap::new();
        map.insert(("openai-codex".into(), "gpt-5.6-luna".into()), 1_000_000);
        map.insert(("openai-codex".into(), "gpt-6-luna".into()), 272_000);
        map.insert(("anthropic".into(), "claude-opus-5".into()), 1_000_000);
        Self(map)
    }
}

impl WindowCatalog for FakeCatalog {
    fn context_window(
        &self,
        _profile: &Profile,
        provider: &str,
        model: &str,
    ) -> Result<Option<u64>, String> {
        Ok(self.0.get(&(provider.to_owned(), model.to_owned())).copied())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn agent(&self, profile: Option<&str>) -> PathBuf {
        let omp = self.path().join(".omp");
        match profile {
            Some(name) => omp.join("profiles").join(name).join("agent"),
            None => omp.join("agent"),
        }
    }

    fn session(&self, profile: Option<&str>, lines: &[serde_json::Value]) -> PathBuf {
        self.named_session(profile, "2026-09-25T17-26-54-990Z_01a0d99b.jsonl", lines)
    }

    /// Write a session file whose entries form one parent chain, as OMP appends them: each
    /// entry's `parentId` is the previous entry's `id` (the `session` header is not in the tree).
    fn named_session(
        &self,
        profile: Option<&str>,
        name: &str,
        lines: &[serde_json::Value],
    ) -> PathBuf {
        let dir = self.agent(profile).join("sessions").join("-Developer-jev");
        fs::create_dir_all(&dir).expect("sessions dir");
        let path = dir.join(name);
        fs::write(&path, chain(lines)).expect("session");
        path
    }

    fn pointer(&self, profile: Option<&str>, key: &str, session: &Path, fresh: bool) {
        let dir = self.agent(profile).join("terminal-sessions");
        fs::create_dir_all(&dir).expect("terminal-sessions dir");
        let extra = if fresh { "fresh\n" } else { "" };
        fs::write(
            dir.join(key),
            format!(
                "/home/op/Developer/jev\n{}\n{extra}cwdstat 16777233 1872379237\n",
                session.display()
            ),
        )
        .expect("pointer");
    }
}

fn chain(lines: &[serde_json::Value]) -> String {
    let mut previous: Option<String> = None;
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        let mut entry = line.clone();
        if entry["type"] != "session" {
            if entry.get("id").is_none() {
                entry["id"] = json!(format!("c{index:04}"));
            }
            entry["parentId"] = previous.clone().map_or(serde_json::Value::Null, |p| json!(p));
            previous = entry["id"].as_str().map(str::to_owned);
        }
        text.push_str(&format!("{entry}\n"));
    }
    text
}

fn pane(argv: &str, started_epoch: u64) -> PaneObservation {
    PaneObservation {
        pane_id: "%27".into(),
        label: "jev:0.3".into(),
        tty: "/dev/ttys004".into(),
        omp: Some(OmpProcess {
            pid: 46883,
            argv: argv.into(),
            started_epoch,
        }),
    }
}

const CODEX: &str = "bun /home/op/.bun/bin/omp --profile codex";

fn header() -> Vec<serde_json::Value> {
    vec![
        json!({"type":"session","version":3,"id":"s","timestamp":"2026-09-25T17:26:54.990Z","cwd":"/home/op/Developer/jev"}),
        json!({"type":"model_change","id":"m1","timestamp":"2026-09-25T17:26:55.189Z","model":"openai-codex/gpt-5.6-luna"}),
    ]
}

fn assistant(ts: &str, prompt: u64, stop: &str) -> serde_json::Value {
    json!({"type":"message","id":ts,"timestamp":ts,"message":{
        "role":"assistant","provider":"openai-codex","model":"gpt-5.6-luna","stopReason":stop,
        "usage":{"input":604,"output":85,"cacheRead":prompt - 604,"cacheWrite":0,"totalTokens":prompt + 85},
        "contextSnapshot":{"promptTokens":prompt,"nonMessageTokens":69640,"compactionEpoch":0}}})
}

fn measured(context: &PaneContext) -> &omp_pane_context::Measured {
    match context {
        PaneContext::Measured(m) => m,
        PaneContext::Unknown { reason, detail } => {
            panic!("expected MEASURED, got {} {detail}", reason.as_str())
        }
    }
}

fn unknown_reason(context: &PaneContext) -> UnknownReason {
    match context {
        PaneContext::Unknown { reason, .. } => *reason,
        PaneContext::Measured(m) => panic!("expected UNKNOWN, got {:.1}%", m.percent),
    }
}

// ---- the acceptance leg: no session file is UNKNOWN, never 0 --------------------------------

#[test]
fn no_session_file_reads_unknown_never_zero() {
    let home = Home::new();
    let missing = home
        .agent(Some("codex"))
        .join("sessions/-Developer-jev/never-materialized.jsonl");
    home.pointer(Some("codex"), "ttys004", &missing, true);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::NoSessionFile);
    assert_eq!(context.percent_text(), UNKNOWN);
    let obs = pane(CODEX, 0);
    assert_eq!(max_percent_text(&[(obs, context)]), UNKNOWN);
}

#[test]
fn session_file_without_any_usage_is_unknown_not_zero() {
    let home = Home::new();
    let session = home.session(Some("codex"), &header());
    home.pointer(Some("codex"), "ttys004", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::NoUsageRecord);
    assert_eq!(context.percent_text(), UNKNOWN);
}

// ---- the reading ------------------------------------------------------------------------------

#[test]
fn newest_valid_anchor_wins_and_aborted_turns_are_skipped() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T17:50:00.000Z", 100_000, "toolUse"));
    lines.push(assistant("2026-09-25T17:58:28.910Z", 178_268, "toolUse"));
    // An aborted turn's usage is never an anchor (transcript-tokens.ts:46).
    lines.push(assistant("2026-09-25T17:59:00.000Z", 900_000, "aborted"));
    lines.push(json!({"type":"message","id":"u","timestamp":"2026-09-25T17:59:01.000Z","message":{"role":"user","content":"hi"}}));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    let m = measured(&context);
    assert_eq!(m.tokens, 178_268);
    assert_eq!(m.context_window, 1_000_000);
    assert_eq!(context.percent_text(), "17.8");
    assert_eq!(m.read_at, "2026-09-25T17:58:28.910Z");
    assert_eq!(m.source, ReadingSource::UsageAnchor);
    assert_eq!(m.session_file, session);
}

#[test]
fn history_rewrite_is_subtracted_and_usage_is_the_fallback() {
    let home = Home::new();
    let mut lines = header();
    let mut rewritten = assistant("2026-09-25T18:00:00.000Z", 300_000, "stop");
    rewritten["message"]["contextSnapshot"]["historyRewriteTokensRemoved"] = json!(50_000);
    lines.push(rewritten);
    let session = home.session(Some("codex"), &lines);
    let scan = last_usage_record(&session).expect("scan");
    let TailScan::Record(record) = scan else { panic!("{scan:?}") };
    assert_eq!(record.tokens, 250_000);

    // No contextSnapshot: calculatePromptTokens = input + cacheRead + cacheWrite.
    let mut bare = header();
    bare.push(json!({"type":"message","timestamp":"2026-09-25T18:01:00.000Z","message":{
        "role":"assistant","provider":"openai-codex","model":"gpt-5.6-luna","stopReason":"stop",
        "usage":{"input":1000,"output":50,"cacheRead":40_000,"cacheWrite":9_000,"totalTokens":50_050}}}));
    let session = home.session(Some("codex"), &bare);
    let TailScan::Record(record) = last_usage_record(&session).expect("scan") else {
        panic!("no record")
    };
    assert_eq!(record.tokens, 50_000);
}

#[test]
fn a_newer_compaction_supersedes_the_anchor_with_tokens_after() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T14:30:00.000Z", 853_646, "toolUse"));
    lines.push(json!({"type":"compaction","id":"c","timestamp":"2026-09-25T14:37:33.746Z",
        "tokensBefore":853_646,"tokensAfter":91_320,"method":"remote","summary":"s"}));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    let m = measured(&context);
    assert_eq!(m.tokens, 91_320);
    assert_eq!(m.source, ReadingSource::CompactionTokensAfter);
    assert_eq!(m.read_at, "2026-09-25T14:37:33.746Z");
    assert_eq!(context.percent_text(), "9.1");
}

#[test]
fn a_model_switch_after_the_anchor_sets_the_window() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 136_000, "stop"));
    lines.push(json!({"type":"model_change","timestamp":"2026-09-25T18:01:00.000Z","model":"openai-codex/gpt-6-luna"}));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    let m = measured(&context);
    assert_eq!((m.model.as_str(), m.context_window), ("gpt-6-luna", 272_000));
    assert_eq!(context.percent_text(), "50.0");
}

#[test]
fn an_unknown_window_is_unknown_even_with_tokens() {
    let home = Home::new();
    let mut lines = header();
    let mut turn = assistant("2026-09-25T18:00:00.000Z", 136_000, "stop");
    turn["message"]["model"] = json!("muse-spark-1.3-contributor");
    lines.push(turn);
    lines.push(json!({"type":"model_change","timestamp":"2026-09-25T18:01:00.000Z","model":"muse-code/muse-spark-1.3-contributor"}));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::WindowUnknown);
    assert_eq!(context.percent_text(), UNKNOWN);
}

// ---- resolution traps -------------------------------------------------------------------------

#[test]
fn a_breadcrumb_older_than_the_live_process_is_stale() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 136_000, "stop"));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    // The process started an hour AFTER the breadcrumb was written: a previous process on a
    // reused TTY wrote it.
    let context = read_pane(&pane(CODEX, now() + 3600), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::StalePointer);
}

#[test]
fn a_tmux_keyed_breadcrumb_is_never_read() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 136_000, "stop"));
    let session = home.session(Some("codex"), &lines);
    // Only the non-interactive key exists (written by a TTY-less child omp).
    home.pointer(Some("codex"), "tmux-%27", &session, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::NoPointer);
}

#[test]
fn a_session_outside_the_profile_store_is_foreign() {
    let home = Home::new();
    let fixture = home.path().join("scratch/rpc-resume-profile/session.jsonl");
    fs::create_dir_all(fixture.parent().expect("parent")).expect("dir");
    fs::write(&fixture, format!("{}\n", assistant("2026-09-25T18:00:00.000Z", 136_000, "stop"))).expect("fixture");
    home.pointer(Some("codex"), "ttys004", &fixture, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::ForeignSessionPath);
}

#[test]
fn the_profile_selects_the_root_and_the_unprofiled_root_is_never_a_fallback() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 136_000, "stop"));
    // The breadcrumb exists only under the UNPROFILED root.
    let session = home.session(None, &lines);
    home.pointer(None, "ttys004", &session, false);
    let profiled = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&profiled), UnknownReason::NoPointer);
    let unprofiled = read_pane(
        &pane("bun /home/op/.bun/bin/omp", now() - 60),
        home.path(),
        &FakeCatalog::standard(),
    );
    assert_eq!(measured(&unprofiled).profile, Profile::Unprofiled);
}

#[test]
fn a_pane_without_omp_is_unknown() {
    let home = Home::new();
    let mut observation = pane(CODEX, 0);
    observation.omp = None;
    let context = read_pane(&observation, home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::NoOmpProcess);
}

// ---- the reverse reader ------------------------------------------------------------------------

#[test]
fn a_line_larger_than_the_read_chunk_and_no_trailing_newline_still_parse() {
    let home = Home::new();
    let dir = home.agent(Some("codex")).join("sessions/-Developer-jev");
    fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("big.jsonl");
    let mut big = assistant("2026-09-25T18:00:00.000Z", 424_242, "stop");
    big["message"]["content"] = json!([{"type":"text","text":"x".repeat(300_000)}]);
    let noise = json!({"type":"custom","customType":"tool_execution_start","data":"y".repeat(150_000)});
    // Big anchor, then a big non-anchor line with NO trailing newline.
    fs::write(&path, format!("{}\n{}\n{}", header()[0], big, noise)).expect("write");
    let TailScan::Record(record) = last_usage_record(&path).expect("scan") else {
        panic!("no record")
    };
    assert_eq!(record.tokens, 424_242);
}

// ---- small parsers --------------------------------------------------------------------------------

#[test]
fn breadcrumb_parser_accepts_the_four_line_omp_18_3_shape() {
    let crumb = parse_breadcrumb(
        "/home/op/Developer/jev\n/x/sessions/a.jsonl\nfresh\ncwdstat 16777233 1872379237\n",
    )
    .expect("four-line breadcrumb");
    assert!(crumb.fresh);
    assert_eq!(crumb.session_file, PathBuf::from("/x/sessions/a.jsonl"));
    assert!(parse_breadcrumb("only-one-line\n").is_none());
}

#[test]
fn launcher_shapes_other_than_bun_omp_are_recognised_and_bystanders_are_not() {
    // The trust-guard wrapper installed as `omp`, a direct node launch, runtime flags, and the
    // resolved bundle path all start an interactive omp.
    for argv in [
        "/home/op/.local/bin/omp --profile claude",
        "omp --profile=glm",
        "node /home/op/.bun/bin/omp --profile grok",
        "bun --smol /home/op/.bun/bin/omp --profile codex",
        "bun /home/op/.bun/install/global/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js --profile muse",
    ] {
        assert!(is_omp_argv(argv), "{argv}");
    }
    // A pager or editor holding a file named omp, a shell, and a bun script are not omp.
    for argv in ["less omp", "vim /tmp/omp", "bun run build", "node server.js omp", "-zsh"] {
        assert!(!is_omp_argv(argv), "{argv}");
    }
    assert_eq!(
        omp_pane_context::profile_from_argv("/home/op/.local/bin/omp --profile claude"),
        Ok(Profile::Profiled("claude".into()))
    );
    assert_eq!(
        omp_pane_context::profile_from_argv("omp --profile=glm"),
        Ok(Profile::Profiled("glm".into()))
    );
}

#[test]
fn the_interactive_omp_is_the_shallowest_not_a_tool_spawned_child() {
    let rows = parse_ps(
        "11832  1369 Ss   1-16:14:05 -zsh\n\
         34593 11832 S+   19:29      bun /home/op/.bun/bin/omp --profile codex\n\
         40000 34593 S    00:05      bun /home/op/.bun/bin/omp --mode=rpc --session /tmp/x.jsonl\n",
    );
    let found = find_omp(&rows, 11832, 10_000).expect("omp under pane");
    assert_eq!(found.pid, 34593);
    assert_eq!(found.started_epoch, 10_000 - 1169);
    assert!(find_omp(&rows, 1369, 10_000).is_some_and(|p| p.pid == 34593));
    assert!(find_omp(&rows, 99_999, 10_000).is_none());
}

#[test]
fn a_zombie_omp_is_not_an_agent_and_never_yields_a_reading() {
    // The agent exited and its parent shell has not reaped it: ps still lists the argv.
    let rows = parse_ps(
        "11832  1369 Ss   1-16:14:05 -zsh\n\
         34593 11832 Z+   19:29      bun /home/op/.bun/bin/omp --profile codex\n",
    );
    assert!(rows[1].is_zombie());
    assert!(find_omp(&rows, 11832, 10_000).is_none());

    // Its breadcrumb and session are fresh and valid, and the pane still reads UNKNOWN.
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 456_000, "stop"));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let mut observation = pane(CODEX, 0);
    observation.omp = find_omp(&rows, 11832, now());
    let context = read_pane(&observation, home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::NoOmpProcess);
}

#[test]
fn terminal_ids_and_elapsed_times_parse_like_omp_and_ps() {
    assert_eq!(terminal_id("/dev/ttys004").as_deref(), Some("ttys004"));
    assert_eq!(terminal_id("/dev/pts/3").as_deref(), Some("pts-3"));
    assert_eq!(terminal_id("ttys004"), None);
    assert_eq!(parse_etime("52:20"), Some(3140));
    assert_eq!(parse_etime("01:02:03"), Some(3723));
    assert_eq!(parse_etime("1-16:14:05"), Some(144_845));
    assert_eq!(parse_etime("61:00"), None);
    assert!(is_omp_argv("bun /home/op/.bun/bin/omp --profile codex"));
    assert!(!is_omp_argv("/home/op/.local/bin/omp-trust-guard"));
    assert!(!is_omp_argv("-zsh"));
}

#[test]
fn session_max_excludes_unknown_panes_instead_of_counting_them_as_zero() {
    let home = Home::new();
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", 456_000, "stop"));
    let session = home.session(Some("codex"), &lines);
    home.pointer(Some("codex"), "ttys004", &session, false);
    let catalog = FakeCatalog::standard();
    let high = pane(CODEX, now() - 60);
    let high_context = read_pane(&high, home.path(), &catalog);
    let mut none = pane(CODEX, 0);
    none.omp = None;
    let none_context = read_pane(&none, home.path(), &catalog);
    assert_eq!(
        max_percent_text(&[(none.clone(), none_context.clone()), (high, high_context)]),
        "45.6"
    );
    assert_eq!(max_percent_text(&[(none, none_context)]), UNKNOWN);
}

// ---- record selection: one expectation file, shared with the independent reader ----------

/// Every synthetic session in `tests/fixtures/selection/` against `expected.json`. The same
/// expectation file is what the out-of-tree independent reader is checked against, so a
/// divergence between the two implementations fails one side or the other.
#[test]
fn selection_fixtures_match_the_shared_expectation_file() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection");
    let expected: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).expect("expected.json"))
            .expect("expected.json parses");
    let expected = expected.as_object().expect("expected.json is an object");
    let mut fixtures: Vec<String> = fs::read_dir(&dir)
        .expect("fixture dir")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".jsonl").map(str::to_owned)
        })
        .collect();
    fixtures.sort();
    // Anti-vacuity: an empty or partially-described fixture set is an error, not a pass.
    assert!(fixtures.len() >= 15, "only {} fixtures", fixtures.len());
    let mut named: Vec<&String> = expected.keys().collect();
    named.sort();
    assert_eq!(fixtures.iter().collect::<Vec<_>>(), named, "fixtures and expectations differ");
    for name in &fixtures {
        let scan = last_usage_record(&dir.join(format!("{name}.jsonl"))).expect("scan");
        let want = &expected[name];
        let got = match scan {
            TailScan::Record(r) => json!({
                "tokens": r.tokens,
                "source": r.source.as_str(),
                "model": format!("{}/{}", r.provider.unwrap_or_default(), r.model.unwrap_or_default()),
            }),
            TailScan::NoUsageRecord => json!({"unknown": "NO_USAGE_RECORD"}),
            TailScan::ResetWithoutUsage { .. } => json!({"unknown": "RESET_WITHOUT_USAGE"}),
            TailScan::CompactionWithoutTokens { .. } => json!({"unknown": "COMPACTION_WITHOUT_TOKENS"}),
        };
        assert_eq!(&got, want, "fixture {name}");
    }
}

// ---- the path boundary --------------------------------------------------------------------

fn anchored_lines(prompt: u64) -> Vec<serde_json::Value> {
    let mut lines = header();
    lines.push(assistant("2026-09-25T18:00:00.000Z", prompt, "stop"));
    lines
}

#[test]
fn a_symlink_inside_sessions_to_a_foreign_file_is_foreign() {
    let home = Home::new();
    let outside = home.path().join("scratch/foreign-fixture.jsonl");
    fs::create_dir_all(outside.parent().expect("parent")).expect("dir");
    fs::write(&outside, chain(&anchored_lines(600_000))).expect("foreign fixture");
    let sessions = home.agent(Some("codex")).join("sessions/-Developer-jev");
    fs::create_dir_all(&sessions).expect("sessions");
    let link = sessions.join("fixture.jsonl");
    std::os::unix::fs::symlink(&outside, &link).expect("symlink");
    home.pointer(Some("codex"), "ttys004", &link, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::ForeignSessionPath);
}

#[test]
fn a_symlinked_directory_and_a_dotdot_path_are_foreign_too() {
    let home = Home::new();
    let outside_dir = home.path().join("scratch/elsewhere");
    fs::create_dir_all(&outside_dir).expect("dir");
    fs::write(outside_dir.join("s.jsonl"), chain(&anchored_lines(600_000))).expect("fixture");
    let sessions = home.agent(Some("codex")).join("sessions");
    fs::create_dir_all(&sessions).expect("sessions");
    std::os::unix::fs::symlink(&outside_dir, sessions.join("-Developer-evil")).expect("dir link");

    home.pointer(Some("codex"), "ttys004", &sessions.join("-Developer-evil/s.jsonl"), false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::ForeignSessionPath);

    // Lexically under `sessions/`, really five levels above it (sessions -> agent -> codex ->
    // profiles -> .omp -> $HOME). The file must EXIST there, or this leg would read
    // NO_SESSION_FILE and prove nothing about containment.
    let dotdot = sessions.join("../../../../../scratch/elsewhere/s.jsonl");
    assert!(dotdot.is_file(), "dot-dot target must exist: {}", dotdot.display());
    assert!(dotdot.starts_with(&sessions), "must pass the lexical check");
    home.pointer(Some("codex"), "ttys004", &dotdot, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(unknown_reason(&context), UnknownReason::ForeignSessionPath);
}

#[test]
fn a_symlink_that_stays_inside_the_profile_store_is_still_read() {
    // Positive control for the two tests above: resolving is not the same as refusing links.
    let home = Home::new();
    let real = home.named_session(Some("codex"), "real.jsonl", &anchored_lines(250_000));
    let link = real.with_file_name("alias.jsonl");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    home.pointer(Some("codex"), "ttys004", &link, false);
    let context = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(measured(&context).tokens, 250_000);
}

// ---- panes and sessions ------------------------------------------------------------------------

#[test]
fn two_panes_in_one_cwd_each_read_their_own_session() {
    let home = Home::new();
    let first = home.named_session(Some("codex"), "a.jsonl", &anchored_lines(100_000));
    let second = home.named_session(Some("codex"), "b.jsonl", &anchored_lines(700_000));
    home.pointer(Some("codex"), "ttys004", &first, false);
    home.pointer(Some("codex"), "ttys010", &second, false);
    let mut left = pane(CODEX, now() - 60);
    left.tty = "/dev/ttys004".into();
    let mut right = pane(CODEX, now() - 60);
    right.pane_id = "%32".into();
    right.tty = "/dev/ttys010".into();
    let catalog = FakeCatalog::standard();
    assert_eq!(measured(&read_pane(&left, home.path(), &catalog)).tokens, 100_000);
    assert_eq!(measured(&read_pane(&right, home.path(), &catalog)).tokens, 700_000);
}

#[test]
fn a_resumed_or_forked_session_is_read_from_the_file_the_breadcrumb_now_names() {
    let home = Home::new();
    // The original session, large; then `/fork` (createBranchedSession) writes a new file with
    // the copied path plus new turns, and rewrites the breadcrumb to it.
    let original = home.named_session(Some("codex"), "original.jsonl", &anchored_lines(800_000));
    let mut forked = anchored_lines(800_000);
    forked.push(json!({"type":"compaction","id":"fc","timestamp":"2026-09-25T18:05:00.000Z","tokensBefore":800_000,"tokensAfter":90_000}));
    forked.push(assistant("2026-09-25T18:06:00.000Z", 120_000, "stop"));
    let fork = home.named_session(Some("codex"), "fork.jsonl", &forked);
    home.pointer(Some("codex"), "ttys004", &original, false);
    let before = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(measured(&before).tokens, 800_000);
    home.pointer(Some("codex"), "ttys004", &fork, false);
    let after = read_pane(&pane(CODEX, now() - 60), home.path(), &FakeCatalog::standard());
    assert_eq!(measured(&after).tokens, 120_000);
    assert_eq!(measured(&after).session_file, fork);
}

#[test]
fn a_session_file_being_appended_to_always_reads_a_complete_anchor() {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let home = Home::new();
    let path = home.named_session(Some("codex"), "live.jsonl", &anchored_lines(100_000));
    let done = Arc::new(AtomicBool::new(false));
    let writer = {
        let (path, done) = (path.clone(), Arc::clone(&done));
        std::thread::spawn(move || {
            let mut file = fs::OpenOptions::new().append(true).open(&path).expect("append");
            let mut parent = "2026-09-25T18:00:00.000Z".to_owned();
            for turn in 1..=400u64 {
                let id = format!("w{turn:04}");
                let mut entry = assistant("2026-09-25T18:10:00.000Z", 100_000 + turn, "toolUse");
                entry["id"] = json!(id);
                entry["parentId"] = json!(parent);
                let line = format!("{entry}\n");
                // Two writes per line: a reader can observe the torn half.
                let (head, tail) = line.as_bytes().split_at(line.len() / 2);
                file.write_all(head).expect("head");
                file.flush().expect("flush");
                file.write_all(tail).expect("tail");
                parent = id;
            }
            done.store(true, Ordering::SeqCst);
        })
    };
    let mut reads = 0u64;
    while !done.load(Ordering::SeqCst) || reads == 0 {
        match last_usage_record(&path).expect("scan") {
            TailScan::Record(record) => {
                assert!((100_000..=100_400).contains(&record.tokens), "{}", record.tokens);
            }
            other => panic!("mid-append read lost the anchor: {other:?}"),
        }
        reads += 1;
    }
    writer.join().expect("writer");
    let TailScan::Record(last) = last_usage_record(&path).expect("scan") else {
        panic!("no record")
    };
    assert_eq!(last.tokens, 100_400);
}

#[test]
fn a_breadcrumb_left_by_a_process_that_since_restarted_is_stale_until_rewritten() {
    let home = Home::new();
    let session = home.session(Some("codex"), &anchored_lines(300_000));
    home.pointer(Some("codex"), "ttys004", &session, false);
    // The old process died; a new omp on the same TTY started later and has not written yet.
    let restarted = pane(CODEX, now() + 600);
    assert_eq!(
        unknown_reason(&read_pane(&restarted, home.path(), &FakeCatalog::standard())),
        UnknownReason::StalePointer
    );
}
