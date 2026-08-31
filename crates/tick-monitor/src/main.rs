//! `tick-monitor` -- three repeatable monitors plus the loop-enforcement choke point.
//!
//!   tick-monitor observe [--session S] [--repo PATH]...   # the three monitors, JSON
//!   tick-monitor emit-tick --mode M --verdict V [...]      # the ONLY sanctioned write
//!   tick-monitor --selftest                                # fires-on-known-bad
//!
//! `observe` is read-only and idempotent apart from the state file it must update to make
//! the next tick's two-capture comparison possible. `--no-save` suppresses even that.

use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;
use tick_monitor::*;

const TMUX_TIMEOUT: Duration = Duration::from_secs(10);
const GIT_TIMEOUT: Duration = Duration::from_secs(20);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--selftest") {
        exit(selftest());
    }
    match args.first().map(String::as_str) {
        Some("observe") => exit(observe(&args[1..])),
        Some("watch") => exit(watch(&args[1..])),
        Some("emit-tick") => exit(emit_tick(&args[1..])),
        Some("lifecycle") => exit(lifecycle(&args[1..])),
        Some("capabilities") => {
            println!("{}", capabilities());
            exit(0)
        }
        _ => {
            eprintln!(
                "usage: tick-monitor [observe|watch|emit-tick|capabilities|--selftest]\n\
                 \n\
                 observe   [--session NAME] [--repo PATH].. [--no-save] [--state PATH]\n\
                 watch     [--interval SECS] [--max-ticks N] [--stall-after N]\n\
                           [--watch-ledger PATH]  + all observe flags\n\
                           THE LOOP: runs observe forever, one JSON line per tick\n\
                 emit-tick --mode MODE --verdict GREEN|RED|BLOCKED\n\
                           [--blocker CLASS:NAME] [--escalation TEXT] [--bead ID]\n\
                           [--note TEXT] [--ledger PATH] [--state PATH]\n"
            );
            exit(2)
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().map(String::as_str);
        }
        if let Some(rest) = a.strip_prefix(&format!("{name}=")) {
            return Some(rest);
        }
    }
    None
}

fn flags<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            if let Some(v) = args.get(i + 1) {
                out.push(v.as_str());
            }
            i += 2;
        } else if let Some(rest) = args[i].strip_prefix(&format!("{name}=")) {
            out.push(rest);
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

fn capabilities() -> String {
    format!(
        "{{\"monitors\":[\"omp_lifecycle\",\"git_commits\",\"idle_panes\"],\
\"modes\":[{}],\"blocker_classes\":[{}],\"min_gap_secs\":{},\
\"exit_codes\":{{\"ok\":0,\"usage\":2,\"observe_empty_scan_set\":3,\
\"unknown_mode\":4,\"forbidden_phrase\":5,\"blocked_contract\":6,\"escalation_required\":7}}}}",
        MODES
            .iter()
            .map(|m| format!("\"{m}\""))
            .collect::<Vec<_>>()
            .join(","),
        BLOCKER_CLASSES
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(","),
        MIN_GAP_SECS
    )
}

// ---------------------------------------------------------------------------
// MONITOR 3: idle panes  (and the pane half of MONITOR 1)
// ---------------------------------------------------------------------------

fn pane_ids(session: &str) -> Result<Vec<String>, String> {
    let out = run(
        &[
            "tmux",
            "list-panes",
            "-a",
            "-F",
            "#{pane_id} #{session_name}",
        ],
        TMUX_TIMEOUT,
    );
    let text = match &out {
        Outcome::Completed { stdout, .. } => stdout,
        // A timeout is not an empty fleet. Refuse rather than report zero panes.
        other => return Err(format!("tmux list-panes {}", other.kind())),
    };
    Ok(text
        .lines()
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            let id = p.next()?;
            let sess = p.next()?;
            (session.is_empty() || sess == session).then(|| id.to_owned())
        })
        .collect())
}

fn capture(pane: &str) -> Option<String> {
    match run(&["tmux", "capture-pane", "-p", "-t", pane, "-S", "-14"], TMUX_TIMEOUT) {
        Outcome::Completed { stdout, .. } => Some(stdout),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// MONITOR 2: git commits
// ---------------------------------------------------------------------------

fn commits_since(repo: &str, since_unix: u64) -> Result<Vec<String>, String> {
    let since = format!("--since=@{since_unix}");
    let out = run(
        &["git", "-C", repo, "log", &since, "--format=%h %s"],
        GIT_TIMEOUT,
    );
    match &out {
        Outcome::Completed { stdout, code, .. } if *code == Some(0) => {
            Ok(stdout.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect())
        }
        other => Err(format!("git log in {repo}: {}", other.kind())),
    }
}

fn head_sha(repo: &str) -> Option<String> {
    match run(&["git", "-C", repo, "rev-parse", "--short", "HEAD"], GIT_TIMEOUT) {
        Outcome::Completed { stdout, code, .. } if code == Some(0) => {
            Some(stdout.trim().to_owned())
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// observe
// ---------------------------------------------------------------------------

fn observe_core(args: &[String]) -> Result<String, i32> {
    let session = flag(args, "--session").unwrap_or("omp-orchestrator");
    let mut repos: Vec<String> = flags(args, "--repo").iter().map(|s| s.to_string()).collect();
    if repos.is_empty() {
        repos.push(std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default());
    }
    // Panes that are never worker capacity. The conductor's own pane belongs here: it is
    // idle between turns by design, so counting it fires the idle alarm forever.
    let excluded: Vec<&str> = flags(args, "--exclude-pane");
    let state_file = flag(args, "--state")
        .map(PathBuf::from)
        .unwrap_or_else(state_path);
    let prior = load(&state_file);
    let now = now_unix();

    let ids = match pane_ids(session) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("REFUSE observe: {e}");
            return Err(3);
        }
    };
    // ANTI-VACUITY: an empty pane set is an ERROR, never a healthy fleet.
    if ids.is_empty() {
        eprintln!("REFUSE observe: pane set is EMPTY for session {session:?}; an empty scan set is an error, never a pass");
        return Err(3);
    }

    let mut obs = Vec::new();
    let mut rows = Vec::new();
    let mut transitions = Vec::new();
    let mut dispatchable = Vec::new();
    let mut free_capacity = Vec::new();

    for id in &ids {
        let Some(cap) = capture(id) else {
            rows.push(format!(
                "{{\"pane\":\"{}\",\"state\":\"UNPROVEN\",\"liveness\":\"UNPROVEN\",\"why\":\"capture_failed\"}}",
                esc(id)
            ));
            continue;
        };
        let state = classify(&cap);
        let o = Observation {
            pane_id: id.clone(),
            state: state.clone(),
            hash: stable_hash(&cap),
            at: now,
        };
        let prev = prior.panes.iter().find(|p| &p.pane_id == id);
        let live = liveness(prev, &o);
        if live.is_dispatchable() && !excluded.contains(&id.as_str()) {
            dispatchable.push(id.clone());
        }
        // Reported separately: a NewlyIdle pane is free capacity a conductor must SEE,
        // even though it may not be filled until the next tick confirms it.
        //
        // The orchestrator's own pane is EXCLUDED. It goes idle between turns by design,
        // and counting it as free capacity fires the idle alarm continuously -- the
        // "LIVENESS counting the orchestrator as a worker" defect named in
        // /ntm-fleet-monitor's reporting contract. Measured: ticks 14 and 15 reported
        // free=['%1397','%1413'], where %1397 is the conductor.
        if live.is_free_capacity() && !excluded.contains(&id.as_str()) {
            free_capacity.push(id.clone());
        }
        if let Some(p) = prev {
            if p.state.label() != state.label() {
                transitions.push(format!(
                    "{{\"pane\":\"{}\",\"from\":\"{}\",\"to\":\"{}\"}}",
                    esc(id),
                    p.state.label(),
                    state.label()
                ));
            }
        }
        let timer = match &state {
            PaneState::Working { timer_secs } => *timer_secs,
            _ => 0,
        };
        let why = match &live {
            Liveness::Unproven { why } => *why,
            _ => "",
        };
        rows.push(format!(
            "{{\"pane\":\"{}\",\"state\":\"{}\",\"timer_secs\":{},\"liveness\":\"{}\",\"why\":\"{}\",\"last_line\":\"{}\"}}",
            esc(id),
            state.label(),
            timer,
            live.label(),
            why,
            esc(last_status_line(&cap))
        ));
        obs.push(o);
    }

    let mut commit_rows = Vec::new();
    let mut new_commits = 0usize;
    let mut heads = Vec::new();
    for repo in &repos {
        let since = if prior.last_tick == 0 { now - 3600 } else { prior.last_tick };
        match commits_since(repo, since) {
            Ok(list) => {
                new_commits += list.len();
                let items: Vec<String> = list.iter().map(|c| format!("\"{}\"", esc(c))).collect();
                commit_rows.push(format!(
                    "{{\"repo\":\"{}\",\"since_unix\":{},\"count\":{},\"commits\":[{}]}}",
                    esc(repo),
                    since,
                    list.len(),
                    items.join(",")
                ));
            }
            Err(e) => commit_rows.push(format!(
                "{{\"repo\":\"{}\",\"error\":\"{}\"}}",
                esc(repo),
                esc(&e)
            )),
        }
        if let Some(h) = head_sha(repo) {
            heads.push((repo.clone(), h));
        }
    }

    let json = format!(
        "{{\"observed_at\":{},\"prior_tick\":{},\"gap_secs\":{},\"session\":\"{}\",\
\"panes_scanned\":{},\
\"idle_panes\":{{\"dispatchable\":[{}],\"count\":{},\"free_capacity\":[{}]}},\
\"omp_lifecycle\":{{\"transitions\":[{}],\"panes\":[{}]}},\
\"git_commits\":{{\"new_total\":{},\"repos\":[{}]}}}}",
        now,
        prior.last_tick,
        now.saturating_sub(prior.last_tick),
        esc(session),
        ids.len(),
        dispatchable
            .iter()
            .map(|d| format!("\"{}\"", esc(d)))
            .collect::<Vec<_>>()
            .join(","),
        dispatchable.len(),
        free_capacity
            .iter()
            .map(|d| format!("\"{}\"", esc(d)))
            .collect::<Vec<_>>()
            .join(","),
        transitions.join(","),
        rows.join(","),
        new_commits,
        commit_rows.join(",")
    );

    if !args.iter().any(|a| a == "--no-save") {
        let next = State {
            last_tick: now,
            last_blocker: prior.last_blocker,
            blocker_streak: prior.blocker_streak,
            red_streak: prior.red_streak,
            panes: obs,
            commits: heads,
        };
        if let Err(e) = save(&state_file, &next) {
            eprintln!("WARN: state not saved ({e}); next tick will read no_prior_capture");
        }
    }
    Ok(json)
}

fn observe(args: &[String]) -> i32 {
    match observe_core(args) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(code) => code,
    }
}

/// MONITOR LOOP. The thing that makes the three monitors "repeatedly available" rather
/// than a command someone has to remember to type.
///
/// Runs `observe_core` forever on an interval, appending one JSON line per tick to a
/// watch ledger so the latest fleet state is readable at any moment without re-running
/// anything. Designed to be supervised (`hub start`), not backgrounded by a shell -- this
/// repo has no `.sh` and a detached shell loop would be exactly the untracked process the
/// substrate is meant to eliminate.
///
/// ESCALATION, mirroring /loop-enforcement's tick-budget rule: consecutive ticks that
/// observe NO commits, NO lifecycle transitions and NO free capacity are no-value ticks.
/// At the configured threshold the loop prints a STALL line naming the count. It does NOT
/// dispatch, notify, or mutate a bead -- a monitor that acts is a second conductor, and
/// this one is deliberately observation-only.
fn watch(args: &[String]) -> i32 {
    let interval: u64 = flag(args, "--interval")
        .and_then(|s| s.parse().ok())
        .unwrap_or(90)
        .max(MIN_GAP_SECS);
    let max_ticks: u64 = flag(args, "--max-ticks")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stall_after: u32 = flag(args, "--stall-after")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    // Two ticks of persistent free capacity is the alarm. One tick is normal -- a worker
    // finishing is the healthy case, and NewlyIdle needs a second capture to confirm
    // anyway. Two means nobody refilled it.
    let capacity_alarm_after: u32 = flag(args, "--capacity-alarm-after")
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let state_file = flag(args, "--state")
        .map(PathBuf::from)
        .unwrap_or_else(state_path);
    let watch_ledger = flag(args, "--watch-ledger")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_file.with_file_name("watch-ledger.jsonl"));
    if let Some(dir) = watch_ledger.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    // The interval is floored at MIN_GAP_SECS on purpose: a shorter loop would make every
    // tick's second capture arrive inside the 75s window and report UNPROVEN forever -- a
    // monitor that can never reach a verdict.
    println!(
        "{{\"watch_started\":{},\"interval_secs\":{},\"min_gap_secs\":{},\"stall_after\":{},\"ledger\":\"{}\"}}",
        now_unix(),
        interval,
        MIN_GAP_SECS,
        stall_after,
        esc(&watch_ledger.display().to_string())
    );

    let mut ticks: u64 = 0;
    let mut no_value: u32 = 0;
    let mut free_streak: u32 = 0;
    loop {
        ticks += 1;
        let line = match observe_core(args) {
            Ok(json) => json,
            Err(code) => {
                // A refusal is a tick too, and it is recorded. An observe that cannot see
                // the fleet must never be silently skipped -- that is how a dead loop
                // looks alive.
                format!(
                    "{{\"observed_at\":{},\"observe_refused\":true,\"exit_code\":{}}}",
                    now_unix(),
                    code
                )
            }
        };

        // A tick has value if anything moved or any capacity opened.
        let has_free = !line.contains("\"free_capacity\":[]");
        let moved = !line.contains("\"new_total\":0")
            || !line.contains("\"transitions\":[]")
            || has_free;
        if moved {
            no_value = 0;
        } else {
            no_value += 1;
        }

        // THE ALARM THIS LOOP WAS MISSING, and it cost three incidents in one shift.
        //
        // `no_value` counts ticks where NOTHING happened. Free capacity is the opposite
        // condition -- something DID happen (a worker finished) -- so every idle pane RESET
        // the stall counter and no alarm ever fired. Measured: %1413 went idle at tick 12
        // and was reported free at ticks 12, 13, 14 and 15 (~6 minutes) with the loop
        // reporting healthy the whole time, because "a worker freed up" reads as motion.
        //
        // Idle capacity is the EXPENSIVE state, not the quiet one. It gets its own streak.
        if has_free {
            free_streak += 1;
        } else {
            free_streak = 0;
        }

        let record = format!(
            "{{\"tick\":{},\"no_value_streak\":{},\"free_capacity_streak\":{},\"observation\":{}}}",
            ticks, no_value, free_streak, line
        );
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&watch_ledger)
        {
            let _ = writeln!(f, "{record}");
        }
        println!("{record}");

        if no_value >= stall_after {
            println!(
                "{{\"stall\":true,\"no_value_streak\":{},\"tick\":{},\"note\":\"{} consecutive ticks with no commit, no transition and no free capacity; a conductor should look, this monitor does not act\"}}",
                no_value, ticks, no_value
            );
        }

        // A single-line file, overwritten every tick, so ONE cheap read answers "is a
        // worker sitting idle right now". The ledger is append-only and grows; nobody
        // greps a growing file under time pressure. This is the file to read first.
        let attention = watch_ledger.with_file_name("ATTENTION.txt");
        let msg = if free_streak >= capacity_alarm_after {
            format!(
                "IDLE CAPACITY for {free_streak} consecutive ticks (tick {ticks}) -- DISPATCH OR GRADE NOW. An idle worker beside a ready queue is the conductor's failure.\n"
            )
        } else if has_free {
            format!("free capacity seen this tick (streak {free_streak}, tick {ticks})\n")
        } else {
            format!("all panes occupied (tick {ticks})\n")
        };
        let _ = std::fs::write(&attention, &msg);
        if free_streak >= capacity_alarm_after {
            // Loud on stdout too, so `hub logs` shows it without parsing JSON.
            print!("{msg}");
        }

        if max_ticks > 0 && ticks >= max_ticks {
            println!("{{\"watch_complete\":true,\"ticks\":{ticks}}}");
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

// ---------------------------------------------------------------------------
// emit-tick -- the choke point
// ---------------------------------------------------------------------------

fn emit_tick(args: &[String]) -> i32 {
    let state_file = flag(args, "--state").map(PathBuf::from).unwrap_or_else(state_path);
    let ledger = flag(args, "--ledger")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_file.with_file_name("tick-ledger.jsonl"));
    let mut prior = load(&state_file);

    let tick = Tick {
        mode: flag(args, "--mode").unwrap_or("").to_owned(),
        verdict: flag(args, "--verdict").unwrap_or("GREEN").to_owned(),
        external_blocker: flag(args, "--blocker").map(str::to_owned),
        escalation_action: flag(args, "--escalation").map(str::to_owned),
        auto_filed_bead: flag(args, "--bead").map(str::to_owned),
        note: flag(args, "--note").unwrap_or("").to_owned(),
    };

    match validate(&tick, &prior.last_blocker, prior.blocker_streak) {
        Err(r) => {
            eprintln!("REJECT [{}] {}", r.code(), r.message());
            r.code()
        }
        Ok(streak) => {
            let red = match tick.verdict.to_ascii_uppercase().as_str() {
                "RED" => prior.red_streak + 1,
                "GREEN" => 0,
                _ => prior.red_streak, // BLOCKED is transparent: neither increments nor resets.
            };
            let line = format!(
                "{{\"ts\":{},\"mode\":\"{}\",\"verdict\":\"{}\",\"external_blocker\":\"{}\",\
\"escalation_action\":\"{}\",\"auto_filed_bead\":\"{}\",\"blocker_streak\":{},\"red_streak\":{},\"note\":\"{}\"}}",
                now_unix(),
                esc(&tick.mode),
                esc(&tick.verdict.to_ascii_uppercase()),
                esc(tick.external_blocker.as_deref().unwrap_or("")),
                esc(tick.escalation_action.as_deref().unwrap_or("")),
                esc(tick.auto_filed_bead.as_deref().unwrap_or("")),
                streak,
                red,
                esc(&tick.note)
            );
            if let Some(dir) = ledger.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            use std::io::Write;
            match std::fs::OpenOptions::new().create(true).append(true).open(&ledger) {
                Ok(mut f) => {
                    if writeln!(f, "{line}").is_err() {
                        eprintln!("REJECT: ledger write failed");
                        return 8;
                    }
                }
                Err(e) => {
                    eprintln!("REJECT: cannot open ledger {}: {e}", ledger.display());
                    return 8;
                }
            }
            prior.last_blocker = tick.external_blocker.unwrap_or_default();
            prior.blocker_streak = streak;
            prior.red_streak = red;
            let _ = save(&state_file, &prior);
            println!("{line}");
            0
        }
    }
}

// ---------------------------------------------------------------------------
// selftest -- both directions, naming which detector fired
// ---------------------------------------------------------------------------

fn selftest() -> i32 {
    let mut fail = 0;
    let mut n = 0;
    macro_rules! leg {
        ($name:expr, $cond:expr) => {{
            n += 1;
            if $cond {
                println!("  ok   {}", $name);
            } else {
                println!("  FAIL {}", $name);
                fail += 1;
            }
        }};
    }

    // --- the four LIVE v18 status lines, captured from real panes 2026-08-31 ---
    let glm = " \u{2819} 12m  \u{b7} \u{25c9} GLM 5.3 \u{b7} \u{1f4c1} ~/Developer/omp-orchestrator \u{b7} \u{2442} main *1 ?6 \u{b7} \u{25eb} 13.0%/1.3M \u{27f2} \u{b7} $1.30";
    let codex = " \u{280f} 6m  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/omp-orchestrator > \u{2442} main *1 ?6 > S0.13 \u{25b6}";
    let opus = " \u{2834} 48s  \u{b7} \u{25d5} Opus 5 \u{b7} \u{1f4c1} ~/Developer/omp-orchestrator \u{b7} \u{2442} main *1 ?6 \u{b7} \u{25eb} 36.2%/1M \u{27f2} \u{b7} S14.76";
    let idle_codex = " \u{3c0}  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/omp-orchestrator > \u{2442} main *1 ?1 > S0.25 \u{25b6}";
    let idle_glm = " \u{3c0}  \u{b7} \u{25c9} GLM 5.3 \u{b7} \u{1f4c1} ~/Developer/omp-orchestrator \u{b7} \u{2442} main *1 ?1 \u{b7} \u{25eb} 17.4%/1.3M \u{27f2} \u{b7} $4.62";
    let shell = "-orchestrator %";

    println!("v18 detector (the exact payload pane-truth scores claims_busy=false on):");
    leg!("glm working", matches!(classify(glm), PaneState::Working { timer_secs: 720 }));
    leg!("codex working", matches!(classify(codex), PaneState::Working { timer_secs: 360 }));
    leg!("opus working", matches!(classify(opus), PaneState::Working { timer_secs: 48 }));
    leg!("codex idle", classify(idle_codex) == PaneState::Idle);
    leg!("glm idle", classify(idle_glm) == PaneState::Idle);
    leg!("shell unproven", classify(shell) == PaneState::Unproven);
    leg!(
        "wedged beats everything",
        classify("Press up to edit queued messages") == PaneState::Wedged
    );

    println!("known-bad: tokens that must NOT read as an elapsed timer");
    leg!("token budget 1.3M is not a timer", parse_timer("13.0%/1.3M").is_none());
    leg!("spend S0.25 is not a timer", parse_timer("S0.25").is_none());
    leg!("uppercase 5M is not a timer", parse_timer("5M").is_none());
    leg!("bare word is not a timer", parse_timer("main").is_none());
    leg!("12m is a timer", parse_timer("12m") == Some(720));

    println!("known-bad: a spinner in SCROLLBACK PROSE must not make a pane working");
    let prose = format!("agent said \"{glm}\" earlier\n{idle_glm}");
    leg!("last-line anchoring defeats prose", classify(&prose) == PaneState::Idle);

    println!("spinner trap: a stripped hash must be stable while only the spinner animates");
    let f1 = format!("body unchanged\n \u{280b} 5m  \u{b7} same");
    let f2 = format!("body unchanged\n \u{2819} 5m  \u{b7} same");
    leg!("animation alone does not change the hash", stable_hash(&f1) == stable_hash(&f2));
    let f3 = "body CHANGED\n \u{280b} 5m  \u{b7} same".to_owned();
    leg!("real content change does change the hash", stable_hash(&f1) != stable_hash(&f3));

    println!("two-capture liveness (never idle from one observation)");
    let mk = |st: PaneState, h: u64, at: u64| Observation {
        pane_id: "%1".to_owned(),
        state: st,
        hash: h,
        at,
    };
    leg!(
        "single capture is UNPROVEN, not idle",
        matches!(liveness(None, &mk(PaneState::Idle, 1, 1000)), Liveness::Unproven { why } if why == "no_prior_capture")
    );
    leg!(
        "gap below 75s is UNPROVEN",
        matches!(
            liveness(Some(&mk(PaneState::Idle, 1, 1000)), &mk(PaneState::Idle, 1, 1030)),
            Liveness::Unproven { why } if why == "gap_too_short"
        )
    );
    leg!(
        "two idle captures 80s apart are dispatchable",
        liveness(Some(&mk(PaneState::Idle, 1, 1000)), &mk(PaneState::Idle, 1, 1080))
            .is_dispatchable()
    );
    leg!(
        "advancing timer is LIVE",
        liveness(
            Some(&mk(PaneState::Working { timer_secs: 60 }, 1, 1000)),
            &mk(PaneState::Working { timer_secs: 120 }, 1, 1080)
        ) == Liveness::Live
    );
    leg!(
        "static timer + static hash across 80s is FROZEN",
        liveness(
            Some(&mk(PaneState::Working { timer_secs: 60 }, 7, 1000)),
            &mk(PaneState::Working { timer_secs: 60 }, 7, 1080)
        ) == Liveness::Frozen
    );
    leg!(
        "static timer but CHANGED content is LIVE (long tool call)",
        liveness(
            Some(&mk(PaneState::Working { timer_secs: 60 }, 7, 1000)),
            &mk(PaneState::Working { timer_secs: 60 }, 9, 1080)
        ) == Liveness::Live
    );
    leg!(
        "working pane is NEVER dispatchable",
        !liveness(
            Some(&mk(PaneState::Working { timer_secs: 60 }, 1, 1000)),
            &mk(PaneState::Working { timer_secs: 120 }, 2, 1080)
        )
        .is_dispatchable()
    );
    leg!(
        "unproven is NEVER dispatchable",
        !liveness(None, &mk(PaneState::Idle, 1, 1000)).is_dispatchable()
    );

    println!("loop-enforcement guard");
    let base = |mode: &str, verdict: &str| Tick {
        mode: mode.to_owned(),
        verdict: verdict.to_owned(),
        ..Default::default()
    };
    leg!(
        "fabricated mode hard-rejects (the 2026-04-19 stall shape)",
        validate(&base("47th_HOLD_silent", "GREEN"), "", 0)
            == Err(Reject::UnknownMode("47th_HOLD_silent".to_owned()))
    );
    leg!("valid mode accepted", validate(&base("DISPATCH", "GREEN"), "", 0).is_ok());
    let mut t = base("DISPATCH", "GREEN");
    t.note = "standing by for work".to_owned();
    leg!(
        "forbidden phrase rejected with code 5",
        validate(&t, "", 0).map_err(|e| e.code()) == Err(5)
    );
    leg!(
        "BLOCKED with no blocker rejects",
        validate(&base("BLOCKED", "BLOCKED"), "", 0) == Err(Reject::MissingBlocker)
    );
    let mut b = base("BLOCKED", "BLOCKED");
    b.external_blocker = Some("nonsense:thing".to_owned());
    leg!(
        "bad blocker class rejects",
        matches!(validate(&b, "", 0), Err(Reject::BadBlockerClass(_)))
    );
    let mut b2 = base("BLOCKED", "BLOCKED");
    b2.external_blocker = Some("infrastructure:rch".to_owned());
    leg!(
        "blocker without escalation artifact rejects",
        validate(&b2, "", 0) == Err(Reject::NoEscalationArtifact)
    );
    let mut b3 = base("BLOCKED", "BLOCKED");
    b3.external_blocker = Some("joshua-decision:pricing".to_owned());
    b3.escalation_action = Some("asked".to_owned());
    leg!(
        "joshua-decision without a bead id rejects",
        validate(&b3, "", 0) == Err(Reject::JoshuaDecisionNeedsBead)
    );
    let mut b4 = base("BLOCKED", "BLOCKED");
    b4.external_blocker = Some("joshua-decision:omp-orchestrator-2z2".to_owned());
    b4.escalation_action = Some("filed bead comment".to_owned());
    leg!("joshua-decision WITH a bead id accepted", validate(&b4, "", 0).is_ok());
    let mut b5 = base("BLOCKED", "BLOCKED");
    b5.external_blocker = Some("infrastructure:rch".to_owned());
    b5.escalation_action = Some("standing by".to_owned());
    leg!(
        "forbidden phrase inside escalation_action still rejects",
        validate(&b5, "", 0).map_err(|e| e.code()) == Err(5)
    );
    let mut d = base("DISPATCH", "RED");
    d.external_blocker = Some("infrastructure:rch".to_owned());
    leg!(
        "3rd tick on the same blocker without escalation rejects with code 7",
        validate(&d, "infrastructure:rch", 2).map_err(|e| e.code()) == Err(7)
    );
    let mut d2 = base("L1_REMEDIATION", "RED");
    d2.external_blocker = Some("infrastructure:rch".to_owned());
    leg!(
        "a *_REMEDIATION mode satisfies the escalation ladder",
        validate(&d2, "infrastructure:rch", 2).is_ok()
    );

    println!("subprocess contract");
    match run(&["/bin/echo", "hello"], Duration::from_secs(5)) {
        Outcome::Completed { stdout, code, .. } => {
            leg!("completed carries stdout", stdout.trim() == "hello");
            leg!("completed carries exit code", code == Some(0));
        }
        other => {
            leg!("echo should complete", false);
            eprintln!("    got {}", other.kind());
        }
    }
    let slow = run(&["/bin/sleep", "30"], Duration::from_millis(600));
    leg!(
        "a deadline yields TimedOut, NOT Completed(non-zero)",
        matches!(slow, Outcome::TimedOut { group_killed: true, .. })
    );
    leg!(
        "a timeout is not readable as output (stdout_if_completed is None)",
        slow.stdout_if_completed().is_none()
    );
    leg!(
        "spawn failure is typed, not a panic",
        matches!(
            run(&["/nonexistent/binary/xyz"], Duration::from_secs(2)),
            Outcome::SpawnFailed { .. }
        )
    );
    // >64KiB on BOTH pipes: the undrained-pipe deadlock.
    let big = run(
        &[
            "/bin/sh",
            "-c",
            "yes 0123456789abcdef | head -c 200000; yes fedcba9876543210 | head -c 200000 1>&2",
        ],
        Duration::from_secs(20),
    );
    match &big {
        Outcome::Completed { stdout, stderr, .. } => {
            leg!("200KB stdout fully drained", stdout.len() >= 200_000);
            leg!("200KB stderr fully drained", stderr.len() >= 200_000);
        }
        other => {
            leg!("200KB on both pipes must not deadlock", false);
            eprintln!("    got {}", other.kind());
        }
    }

    println!("\n{} legs, {} failed", n, fail);
    if fail == 0 {
        println!("SELFTEST PASS");
        0
    } else {
        println!("SELFTEST FAIL");
        1
    }
}


/// The four-surface JOIN. Thin on purpose: the logic lives in `lifecycle::collect` where
/// `cargo test` can reach it, not in a `main.rs` subcommand only a human invokes.
fn lifecycle(args: &[String]) -> i32 {
    // NO HARDCODED ROOTS. This function previously fell back to two literal paths, which
    // landed in 228f42a and turned the path-literal gate RED for the whole workspace --
    // blocking another pane's extraction verification. A hardcoded root compiles after a
    // move and then silently reads the wrong repo (-7ai / -npq). Precedence is
    // --repo > OMP_LIFECYCLE_REPOS > upward .git/.beads walk > TYPED ERROR.
    let explicit = flags(args, "--repo");
    let repos = match tick_monitor::resolve_repos(&explicit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("REFUSE lifecycle: {e}");
            return 64;
        }
    };
    match tick_monitor::lifecycle::collect(&repos) {
        Ok(report) => {
            print!("{}", tick_monitor::lifecycle::render(&report));
            0
        }
        Err(why) => {
            eprintln!("REFUSE lifecycle: {why}");
            3
        }
    }
}
