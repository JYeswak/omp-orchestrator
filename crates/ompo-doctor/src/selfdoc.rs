//! `ompo` self-documentation — the five canonical surfaces.
//!
//! Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv
//!
//! ## Why this module exists
//!
//! `canonical-cli-scoping` graded `ompo` at **2 pass, 11 fail**
//! (`agent_ergonomics_audit/audit/phase1_canonical_cli_scoping.txt`). Five of the eleven
//! failures are self-documentation surfaces, and the checker probes them by *executing*
//! them, not by grepping for them:
//!
//! ```text
//! info_surface            ompo --info               rc 0
//! examples_surface        ompo --examples           rc 0   (or `ompo examples --help`)
//! quickstart_command      ompo quickstart --help    rc 0
//! topic_help              ompo help --help          rc 0   <- rc 2 before this module
//! completion_subcommand   ompo completion --help    rc 0
//! ```
//!
//! ## The two design constraints that shaped it
//!
//! **1. NON-SHADOWING.** `ompo help <name>` was adapter-scoped: `help` resolved names from
//! the 86-entry roster and nothing else. Topic help must be *added* without capturing a
//! name the adapter path owns, so [`dispatch`] returns [`None`] for anything that is not a
//! declared topic and the existing adapter path runs untouched.
//! `topics_never_shadow_an_adapter_name` proves the sets are disjoint, and carries a
//! positive control so an empty roster cannot fake the property.
//!
//! **2. NO SECOND PROVENANCE IMPLEMENTATION.** `--info` must report the build commit and
//! source revision, whose *absence* is the measured reason an 86-vs-88 registry drift went
//! unnoticed for a day. That data has an owner —
//! [`crate::provenance::BuildProvenance`] — so this module CONSUMES it.
//! `info_consumes_the_shared_provenance_kernel` asserts the payload equals
//! `BuildProvenance::current()` field for field, so a divergent local copy fails the suite
//! rather than drifting quietly.
//!
//! ## NO-CLAIM
//!
//! These are documentation surfaces. They report what `ompo` *is*; they verify nothing
//! about whether it works. `--info` reporting `build_commit=unknown` is an honest report of
//! a binary built without the stamping env vars, **not** evidence the binary is stale — that
//! discrimination belongs to `provenance`'s registry probe, not here.

use crate::provenance::BuildProvenance;
use crate::umbrella;

/// Minimum body length for a topic. The canonical scorer's
/// `self_doc_content_minimums` dimension rejects one-line stubs, so the floor is
/// mechanical rather than advisory.
pub const TOPIC_BODY_MINIMUM: usize = 200;

/// Minimum number of curated workflows. The dimension asks for workflows an operator
/// would actually run, not a flag list, so the floor is on *examples* and each one
/// carries its own rationale.
pub const EXAMPLES_MINIMUM: usize = 5;

/// Shells with a real completion emitter. An unlisted shell is a typed refusal, never a
/// silently empty script — an empty completion file installs cleanly and then does
/// nothing, which is the failure class this list exists to prevent.
pub const SHELLS: &[&str] = &["bash", "zsh", "fish"];

/// A help topic: conceptual documentation addressed by name.
///
/// Deliberately NOT an adapter and NOT a verb. `help <adapter>` answers "does this name
/// resolve"; `help <topic>` answers "how does this work".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Topic {
    pub name: &'static str,
    pub summary: &'static str,
    pub body: &'static str,
}

/// A curated workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example {
    pub command: &'static str,
    pub why: &'static str,
}

/// Topic names are chosen to be conceptual nouns precisely because adapter names are
/// workspace bin targets; `topics_never_shadow_an_adapter_name` is what enforces it.
pub const TOPICS: &[Topic] = &[
    Topic {
        name: "probes",
        summary: "what `ompo doctor` measures and how each verdict is reached",
        body: "\
`ompo doctor` runs a fixed probe set and emits one lifecycle event per probe.

Each probe spawns a tool under a bounded deadline and maps the outcome to a typed verdict.
The mapping is the important part, because two of the states are routinely conflated:

  completed + success   the tool is present and answered
  completed + failure   the tool ran and reported a problem
  timed out             UNMEASURED -- the instrument could not answer
  could not spawn       ABSENT_SPECIFIC -- the tool is not there

UNMEASURED and ABSENT are NOT interchangeable. `install the tool` and `the probe could not
read the answer` have opposite remedies, and reporting the first when the second is true
sends an operator to fix something that is not broken. A timeout is not a verdict.

The bounded spawn comes from `subprocess-contract`; the deadline is per probe, so one
hanging tool cannot stall the run.",
    },
    Topic {
        name: "journal",
        summary: "the lifecycle journal, what is stable in it, and what drifts",
        body: "\
`ompo doctor` appends one lifecycle event per probe to a durable journal under the
repository it was pointed at, with fsync plus readback.

The journal is APPEND-ONLY and monotone by design. That has a consequence which has
already cost one wrong acceptance criterion:

  probes            stable per run    -- assertable
  lifecycle_events  stable per run    -- assertable
  readback_lines    GROWS every run   -- NOT assertable

`readback_lines` reports the whole journal length after the append, so four consecutive
runs against one repository report 11, 22, 33, 44. Any test pinning it is red by
construction on the next run. Key acceptance legs on `probes` and `lifecycle_events`.

Use `--repo <path>` to journal somewhere other than the working repository. The journal
directory is not git-ignored by default, so pointing a run at a shared checkout litters
it.",
    },
    Topic {
        name: "scopes",
        summary: "how `--scope` and `--repo` differ, and which one you want",
        body: "\
Two flags select what a run looks at, and they are not alternatives:

  --repo <path>    WHERE the run reads and journals. Changes the subject.
  --scope <name>   WHICH probe family runs within that subject.

`--repo` is the isolation flag. A run against a scratch directory writes its journal
there, which is how you measure doctor behaviour without mutating a shared tree.

`--scope` narrows the probe set. An unsupported scope is a typed refusal naming the
scope, not an empty run -- an empty probe set would report identically to a passing one,
which is the vacuous-pass class.",
    },
    Topic {
        name: "adapters",
        summary: "the adapter roster, addressability, and why that is not execution",
        body: "\
`ompo` is an umbrella over workspace bin targets. `ompo help <adapter>` resolves a name
against the roster and reports whether it is addressable.

ADDRESSABLE IS NOT EXECUTABLE. A resolved name tells you the umbrella knows about the
target; it says nothing about whether the umbrella can run it, and today it cannot --
`ompo doctor --adapter <name>` returns a typed placeholder naming the missing capability
rather than pretending to have probed.

An unknown name must be refused as absent from the roster, not reported as merely
unimplemented: `not a thing` and `a thing we have not built yet` have different remedies.

`ompo capabilities --json` reports the roster size and the verb list. Every reported name
should resolve and every resolvable name should be reported; a gap in either direction is
a defect in the self-report, not a cosmetic issue.",
    },
    Topic {
        name: "provenance",
        summary: "how to tell whether the `ompo` you are running matches your source",
        body: "\
An installed binary and a source tree drift, and the drift is invisible unless the binary
says what it was built from.

`ompo --info` reports package version, build commit and source revision. When the build
was not stamped, the commit and revision read `unknown` -- which is an honest report of a
missing stamp, NOT a claim that the binary is current.

This matters because it is measurable: a registry baked at build time reported 86 adapters
while the workspace held 88 bin targets, and nothing surfaced the difference for a day.
A binary that cannot state its own origin cannot be diagnosed as stale.

Stamp a build by setting OMPO_BUILD_COMMIT and OMPO_SOURCE_REVISION at compile time.",
    },
];

/// Curated workflows. Every command here is a real invocation, not a flag enumeration.
pub const EXAMPLES: &[Example] = &[
    Example {
        command: "ompo doctor --json",
        why: "The default health read. One lifecycle event per probe, machine-readable, \
              and the fastest way to see whether the host has the tools ompo expects.",
    },
    Example {
        command: "ompo doctor --repo /tmp/scratch-repo --json",
        why: "Measure doctor behaviour WITHOUT journalling into a shared checkout. The \
              journal follows --repo, so this is the isolation form to use in a tree \
              other agents are editing.",
    },
    Example {
        command: "ompo init --json",
        why: "Stamp a repository. Refuses with a typed reason when the control files are \
              absent and creates nothing on refusal, so it is safe to run against a repo \
              you do not own to find out whether it would work.",
    },
    Example {
        command: "ompo capabilities --json",
        why: "The self-report: verbs, adapter count, probe count. Use it to check that \
              what ompo claims to offer is what it actually dispatches.",
    },
    Example {
        command: "ompo help pane-truth",
        why: "Resolve one adapter name against the roster. Answers addressability only -- \
              whether the umbrella knows the name, not whether it can run it.",
    },
    Example {
        command: "ompo help probes",
        why: "Topic help, distinct from adapter help. Explains how a verdict is reached, \
              including why UNMEASURED and ABSENT are not interchangeable.",
    },
    Example {
        command: "ompo completion bash | bash -n -",
        why: "Emit the completion script AND check it parses before installing it. A \
              broken completion script installs silently and then fails at the prompt.",
    },
];

/// `ompo --info`: one-shot machine-readable identity dump.
///
/// Consumes [`BuildProvenance`] rather than re-deriving it; see the module docs.
#[must_use]
pub fn info_json() -> serde_json::Value {
    let provenance = BuildProvenance::current();
    umbrella::envelope(
        "info",
        "OK",
        serde_json::json!({
            "package_version": provenance.package_version,
            "build_commit": provenance.build_commit,
            "source_revision": provenance.source_revision,
            "verbs": umbrella::VERBS,
            "adapter_count": umbrella::adapters().len(),
            "topics": TOPICS.iter().map(|topic| topic.name).collect::<Vec<_>>(),
            "shells": SHELLS,
            "journal_relative_path": ".omp-orchestrator/work/s1/lifecycle.jsonl",
            "env_vars": ["OMPO_BUILD_COMMIT", "OMPO_SOURCE_REVISION"],
        }),
    )
}

/// Human-readable `--info`.
#[must_use]
pub fn info_text() -> String {
    let provenance = BuildProvenance::current();
    let mut out = String::new();
    out.push_str("ompo -- umbrella over the omp-orchestrator workspace\n\n");
    out.push_str(&format!("  package_version   {}\n", provenance.package_version));
    out.push_str(&format!("  build_commit      {}\n", provenance.build_commit));
    out.push_str(&format!("  source_revision   {}\n", provenance.source_revision));
    out.push_str(&format!("  verbs             {}\n", umbrella::VERBS.join(", ")));
    out.push_str(&format!("  adapters          {}\n", umbrella::adapters().len()));
    out.push_str(&format!(
        "  topics            {}\n",
        TOPICS
            .iter()
            .map(|topic| topic.name)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!("  completion shells {}\n", SHELLS.join(", ")));
    out.push_str("  journal           .omp-orchestrator/work/s1/lifecycle.jsonl\n");
    out.push_str("  build env vars    OMPO_BUILD_COMMIT, OMPO_SOURCE_REVISION\n");
    if provenance.build_commit == "unknown" || provenance.source_revision == "unknown" {
        out.push_str(
            "\n  NOTE: this build was not stamped, so it cannot state its own origin.\n\
             \x20       `unknown` reports a missing stamp -- it does NOT mean the binary is current.\n",
        );
    }
    out
}

/// `ompo --examples`.
#[must_use]
pub fn examples_text() -> String {
    let mut out = String::from("ompo -- curated workflows\n\n");
    for example in EXAMPLES {
        out.push_str(&format!("  $ {}\n", example.command));
        for line in wrap(example.why, 82) {
            out.push_str(&format!("      {line}\n"));
        }
        out.push('\n');
    }
    out.push_str("More: `ompo help <topic>` for concepts, `ompo quickstart` to start.\n");
    out
}

/// `ompo quickstart`.
#[must_use]
pub fn quickstart_text() -> String {
    let mut out = String::from("ompo quickstart -- for an operator or agent starting cold\n\n");
    out.push_str(
        "1. Find out what you are running.\n\
         \x20     ompo --info\n\
         \x20  Reports the package version plus the build commit and source revision. If those\n\
         \x20  read `unknown` the build was not stamped, so you cannot tell it from any other\n\
         \x20  build of the same version -- that is worth knowing before you trust a result.\n\n",
    );
    out.push_str(
        "2. Read the host.\n\
         \x20     ompo doctor --json\n\
         \x20  One lifecycle event per probe. Read the verdicts, not just the exit code:\n\
         \x20  UNMEASURED means the probe could not answer, which is NOT the same as ABSENT.\n\n",
    );
    out.push_str(
        "3. Measure without side effects.\n\
         \x20     ompo doctor --repo /tmp/scratch --json\n\
         \x20  The journal follows --repo. Use this in a tree someone else is editing; the\n\
         \x20  journal directory is not git-ignored by default.\n\n",
    );
    out.push_str(
        "4. See what ompo claims to be.\n\
         \x20     ompo capabilities --json\n\
         \x20  Verbs, adapter count, probe count. Compare it against what actually dispatches.\n\n",
    );
    out.push_str(
        "5. Learn a concept rather than a flag.\n\
         \x20     ompo help probes\n\
         \x20  Topic help. `ompo help <adapter>` is a different surface: it resolves a name\n\
         \x20  against the roster and answers addressability only.\n\n",
    );
    out.push_str(
        "6. Install completion, and verify it parses first.\n\
         \x20     ompo completion bash | bash -n - && ompo completion bash >> ~/.bashrc\n\n",
    );
    out.push_str("Then: `ompo --examples` for workflows, `ompo help --help` for the topic list.\n");
    out
}

/// `ompo help --help`: the topic index. This is the surface that returned rc 2 before this
/// module existed, and it is deliberately distinct from `help <adapter>`.
#[must_use]
pub fn help_index() -> String {
    let mut out = String::from(
        "ompo help -- two distinct surfaces, neither shadowing the other\n\n\
         \x20 ompo help <topic>     conceptual documentation (listed below)\n\
         \x20 ompo help <adapter>   resolve a name against the adapter roster\n\n\
         Topics:\n",
    );
    let width = TOPICS.iter().map(|t| t.name.len()).max().unwrap_or(0);
    for topic in TOPICS {
        out.push_str(&format!(
            "  {:width$}  {}\n",
            topic.name,
            topic.summary,
            width = width
        ));
    }
    out.push_str(&format!(
        "\nAdapters: {} addressable. `ompo capabilities --json` lists the roster.\n",
        umbrella::adapters().len()
    ));
    out
}

/// Render one topic.
#[must_use]
pub fn topic_text(topic: &Topic) -> String {
    format!(
        "ompo help {} -- {}\n\n{}\n",
        topic.name, topic.summary, topic.body
    )
}

/// Look up a topic by exact name.
#[must_use]
pub fn topic(name: &str) -> Option<&'static Topic> {
    TOPICS.iter().find(|topic| topic.name == name)
}

/// `ompo completion <shell>`.
///
/// # Errors
///
/// Returns a typed refusal naming the shell and the supported set. An unsupported shell
/// must never yield an empty script: an empty completion file installs cleanly and then
/// silently does nothing.
pub fn completion(shell: &str) -> Result<String, String> {
    if !SHELLS.contains(&shell) {
        return Err(format!(
            "SELFDOC_UNSUPPORTED_SHELL shell={shell:?} supported={}",
            SHELLS.join(",")
        ));
    }
    let verbs = umbrella::VERBS.join(" ");
    let topics = TOPICS
        .iter()
        .map(|topic| topic.name)
        .collect::<Vec<_>>()
        .join(" ");
    let shells = SHELLS.join(" ");
    Ok(match shell {
        "bash" => format!(
            "# ompo bash completion\n\
             _ompo() {{\n\
             \x20 local cur prev\n\
             \x20 cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n\
             \x20 prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"\n\
             \x20 case \"$prev\" in\n\
             \x20   help) COMPREPLY=( $(compgen -W \"{topics}\" -- \"$cur\") ); return 0 ;;\n\
             \x20   completion) COMPREPLY=( $(compgen -W \"{shells}\" -- \"$cur\") ); return 0 ;;\n\
             \x20 esac\n\
             \x20 COMPREPLY=( $(compgen -W \"{verbs} --info --examples --json --help\" -- \"$cur\") )\n\
             }}\n\
             complete -F _ompo ompo\n"
        ),
        "zsh" => format!(
            "#compdef ompo\n\
             # ompo zsh completion\n\
             _ompo() {{\n\
             \x20 local -a verbs topics shells\n\
             \x20 verbs=({verbs} --info --examples --json --help)\n\
             \x20 topics=({topics})\n\
             \x20 shells=({shells})\n\
             \x20 if (( CURRENT == 3 )); then\n\
             \x20   case \"${{words[2]}}\" in\n\
             \x20     help) compadd -a topics; return ;;\n\
             \x20     completion) compadd -a shells; return ;;\n\
             \x20   esac\n\
             \x20 fi\n\
             \x20 compadd -a verbs\n\
             }}\n\
             compdef _ompo ompo\n"
        ),
        _ => format!(
            "# ompo fish completion\n\
             complete -c ompo -f\n\
             for verb in {verbs}\n\
             \x20 complete -c ompo -n __fish_use_subcommand -a $verb\n\
             end\n\
             for topic in {topics}\n\
             \x20 complete -c ompo -n '__fish_seen_subcommand_from help' -a $topic\n\
             end\n\
             for shell in {shells}\n\
             \x20 complete -c ompo -n '__fish_seen_subcommand_from completion' -a $shell\n\
             end\n\
             complete -c ompo -l info -d 'identity dump'\n\
             complete -c ompo -l examples -d 'curated workflows'\n"
        ),
    })
}

/// Usage for the `completion` verb itself, so `ompo completion --help` exits 0.
#[must_use]
pub fn completion_usage() -> String {
    format!(
        "ompo completion <shell> -- emit a shell completion script\n\n\
         \x20 shells: {}\n\n\
         Verify before installing; a broken script installs silently and fails at the prompt:\n\
         \x20 ompo completion bash | bash -n -\n",
        SHELLS.join(", ")
    )
}

/// Single entry point, so the binary needs ONE call site.
///
/// Returns [`None`] for anything this module does not own, so the existing adapter-scoped
/// `help` path and every other verb keep their current behaviour. That is what makes the
/// topic surface additive rather than shadowing.
#[must_use]
pub fn dispatch(command: &str, rest: &[String]) -> Option<u8> {
    let wants_help = rest
        .iter()
        .any(|arg| arg == "--help" || arg == "-h");
    let wants_json = rest.iter().any(|arg| arg == "--json");
    match command {
        "--info" => {
            if wants_json {
                println!("{}", info_json());
            } else {
                print!("{}", info_text());
            }
            Some(0)
        }
        "--examples" | "examples" => {
            print!("{}", examples_text());
            Some(0)
        }
        "quickstart" => {
            print!("{}", quickstart_text());
            Some(0)
        }
        "completion" => {
            let shell = rest.iter().find(|arg| !arg.starts_with('-'));
            match shell {
                None => {
                    // Bare `completion` cannot know which shell is meant, and `--help` must
                    // succeed for the canonical probe. Both print usage; only the bare form
                    // is a refusal, because guessing a shell would emit a script for the
                    // wrong one.
                    print!("{}", completion_usage());
                    Some(u8::from(!wants_help) * 2)
                }
                Some(shell) => match completion(shell) {
                    Ok(script) => {
                        print!("{script}");
                        Some(0)
                    }
                    Err(error) => {
                        eprintln!("ompo completion: {error}");
                        Some(2)
                    }
                },
            }
        }
        "help" => {
            // `help --help` is the topic index. A declared topic renders. ANYTHING ELSE
            // returns None so the adapter path keeps the name.
            if rest.is_empty() || wants_help {
                print!("{}", help_index());
                return Some(0);
            }
            let name = rest.first()?;
            let found = topic(name)?;
            print!("{}", topic_text(found));
            Some(0)
        }
        _ => None,
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::process::Command;
    use subprocess_contract::{bounded_output, BoundedOutcome};

    /// CONSTRAINT 1. `help <topic>` must not capture a name the adapter path owns.
    ///
    /// Carries a positive control: an empty roster would make the disjointness trivially
    /// true, so the roster is asserted non-empty first. Otherwise this leg passes
    /// vacuously exactly when the roster is broken.
    #[test]
    fn topics_never_shadow_an_adapter_name() {
        let adapters: BTreeSet<&str> = umbrella::adapters().iter().copied().collect();
        assert!(
            !adapters.is_empty(),
            "POSITIVE CONTROL: an empty roster makes disjointness vacuous"
        );
        assert!(!TOPICS.is_empty(), "ANTI-VACUITY: no topics declared");
        let collisions: Vec<&str> = TOPICS
            .iter()
            .map(|topic| topic.name)
            .filter(|name| adapters.contains(name))
            .collect();
        assert!(
            collisions.is_empty(),
            "topic names shadow adapter names: {collisions:?} -- \
             `help <topic>` would capture a name the adapter path owns"
        );
    }

    /// `dispatch` must decline every adapter name, or the additive property is a claim
    /// rather than a mechanism.
    #[test]
    fn dispatch_declines_adapter_names_so_the_existing_path_runs() {
        for adapter in umbrella::adapters() {
            let rest = vec![(*adapter).to_owned()];
            assert_eq!(
                dispatch("help", &rest),
                None,
                "dispatch captured adapter {adapter:?}; the adapter path can never run"
            );
        }
    }

    /// CONSTRAINT 2. `--info` consumes the shared provenance kernel. A divergent second
    /// implementation fails here rather than drifting quietly.
    #[test]
    fn info_consumes_the_shared_provenance_kernel() {
        let expected = BuildProvenance::current();
        let value = info_json();
        let data = &value["data"];
        assert_eq!(data["package_version"], expected.package_version);
        assert_eq!(data["build_commit"], expected.build_commit);
        assert_eq!(data["source_revision"], expected.source_revision);
    }

    /// The canonical scorer rejects stubs, so the floor is mechanical.
    #[test]
    fn every_topic_body_meets_the_content_minimum() {
        for topic in TOPICS {
            assert!(
                topic.body.len() >= TOPIC_BODY_MINIMUM,
                "topic {:?} body is {} bytes, below the {} minimum -- a stub reports \
                 identically to real documentation",
                topic.name,
                topic.body.len(),
                TOPIC_BODY_MINIMUM
            );
            assert!(!topic.summary.is_empty(), "topic {:?} has no summary", topic.name);
        }
    }

    /// Workflows, not a flag list: every example is a real invocation with a rationale.
    #[test]
    fn examples_are_curated_workflows_with_rationale() {
        assert!(
            EXAMPLES.len() >= EXAMPLES_MINIMUM,
            "{} examples, below the {EXAMPLES_MINIMUM} minimum",
            EXAMPLES.len()
        );
        for example in EXAMPLES {
            assert!(
                example.command.starts_with("ompo "),
                "example {:?} is not an ompo invocation",
                example.command
            );
            assert!(
                example.why.len() >= 40,
                "example {:?} has no substantive rationale",
                example.command
            );
        }
    }

    /// Each of the five canonical probes, exercised through `dispatch` at the exact
    /// argument shape the checker uses.
    #[test]
    fn every_canonical_probe_shape_succeeds() {
        let help = vec!["--help".to_owned()];
        assert_eq!(dispatch("--info", &[]), Some(0));
        assert_eq!(dispatch("--examples", &[]), Some(0));
        assert_eq!(dispatch("quickstart", &help), Some(0));
        assert_eq!(dispatch("help", &help), Some(0), "`ompo help --help` was rc 2");
        assert_eq!(dispatch("completion", &help), Some(0));
    }

    /// KNOWN-BAD. An unsupported shell is a typed refusal naming the shell, never an
    /// empty script.
    #[test]
    fn an_unsupported_shell_is_a_typed_refusal_not_an_empty_script() {
        let error = completion("nonesuch").expect_err("must refuse");
        assert!(
            error.contains("SELFDOC_UNSUPPORTED_SHELL"),
            "refusal must name its class: {error}"
        );
        assert!(error.contains("nonesuch"), "refusal must name the shell: {error}");
        assert_eq!(dispatch("completion", &["nonesuch".to_owned()]), Some(2));
    }

    /// A bare `completion` cannot know the shell, so it refuses rather than guessing --
    /// while `--help` still succeeds for the canonical probe.
    #[test]
    fn bare_completion_refuses_while_help_succeeds() {
        assert_eq!(dispatch("completion", &[]), Some(2));
        assert_eq!(dispatch("completion", &["--help".to_owned()]), Some(0));
    }

    /// KNOWN-GOOD, and the acceptance-B leg. Every emitted script must PARSE. A completion
    /// script with broken syntax installs cleanly and then fails at the prompt, which is
    /// silent in exactly the place an operator cannot see it.
    ///
    /// Spawns through `subprocess_contract::bounded_output` -- the crate's own bounded
    /// spawn -- rather than a raw wait, so a hanging shell cannot stall the suite.
    #[test]
    fn emitted_bash_and_zsh_scripts_parse_under_the_shell() {
        for shell in ["bash", "zsh"] {
            let script = completion(shell).expect("script");
            let mut command = Command::new(shell);
            command.arg("-n").arg("-c").arg(&script);
            command.stdin(std::process::Stdio::null());
            match bounded_output(&mut command, std::time::Duration::from_secs(20)) {
                BoundedOutcome::Completed(output) => assert!(
                    output.status.success(),
                    "{shell} rejected the emitted script: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                // UNMEASURED, not a pass: an absent or hanging shell cannot certify syntax.
                BoundedOutcome::TimedOut => {
                    panic!("{shell} -n timed out; syntax is UNMEASURED, which is not a pass")
                }
                BoundedOutcome::Unspawned(error) => {
                    eprintln!("SKIP {shell}: not installed here ({error}) -- UNMEASURED");
                }
            }
        }
    }

    /// The journal-drift rule this module documents is the one that produced a wrong
    /// acceptance criterion, so the topic must actually carry it.
    #[test]
    fn the_journal_topic_states_which_fields_are_assertable() {
        let journal = topic("journal").expect("journal topic");
        assert!(journal.body.contains("readback_lines"));
        assert!(
            journal.body.contains("NOT assertable"),
            "the journal topic must say which field cannot be pinned"
        );
    }
}
