use super::{
    fixture,
    projection::project,
    replay::run,
    scenario::{Mutation, Scenario},
};

use super::assertions::{check, stable};

#[test]
fn recorded_registry_runs_every_provider_through_store_and_rows() {
    let mut count = 0;
    for entry in std::fs::read_dir(fixture::directory()).unwrap() {
        let path = entry.unwrap().path();
        let Some(name) = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".manifest.json")
        else {
            continue;
        };
        verify_fixture(name);
        count += 1;
    }
    assert_eq!(count, 71, "every fixture needs a registered runner");
}

fn verify_fixture(name: &str) {
    let scenario = Scenario::recorded(name);
    let provider = scenario.fixture.provider;
    let sends = scenario.fixture.expected_outbound.len();
    let handoff = super::handoff_recordings::NAMES.contains(&name);
    let requests = super::codex_requests::NAMES
        .iter()
        .any(|(n, ..)| *n == name);
    let claude_requests = super::claude_requests::NAMES
        .iter()
        .any(|(n, ..)| *n == name);
    let agy_requests = super::agy_requests::NAMES.iter().any(|(n, ..)| *n == name);
    let replay = if super::questions::NAMES.contains(&name) {
        super::questions::recorded(name)
    } else if handoff {
        super::handoff_recordings::recorded(name)
    } else if requests {
        super::codex_requests::recorded(name)
    } else if claude_requests {
        super::claude_requests::recorded(name)
    } else if agy_requests {
        super::agy_requests::recorded(name)
    } else {
        run(scenario)
    };
    assert_eq!(replay.items, replay.reopened, "reopened SQL: {name}");
    assert_eq!(replay.outbound.len(), sends, "only recorded send: {name}");
    assert!(!replay.busy, "queue settled: {name}");
    let projection = project(&replay);
    // Synthetic source runs precede the captured turns in SQL, not on the
    // target's event stream; only the recorded target rows project live.
    let live_items: Vec<_> =
        replay
            .items
            .iter()
            .filter(|item| {
                !handoff
                    || replay.ledger["runs"].as_array().unwrap().iter().any(|run| {
                        run["id"] == item["run_id"] && run["provider"] == provider.as_str()
                    })
            })
            .cloned()
            .collect();
    assert_eq!(
        stable(&live_items),
        stable(projection["items"].as_array().unwrap()),
        "live/reload: {name}"
    );
    let ids: Vec<i64> = replay
        .items
        .iter()
        .map(|v| v["id"].as_i64().unwrap())
        .collect();
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "monotonic committed row ids: {name}"
    );
    assert_eq!(replay.snapshots.len(), replay.envelopes.len());
    assert!(replay.schema.contains(&"harness_items".to_string()));
}

#[test]
fn baseline_mutations_are_driven_through_the_same_pipeline() {
    for mutation in [
        Mutation::None,
        Mutation::EditedFinal,
        Mutation::AnswerBeforeTool,
        Mutation::RepeatSnapshot,
        Mutation::MissingTerminal,
        Mutation::PublicNarration,
        Mutation::Artifact,
    ] {
        let replay = run(Scenario::recorded("claude-144-duplicate-a").mutate(mutation));
        let projection = project(&replay);
        assert_eq!(replay.items, replay.reopened);
        assert!(projection["rows"].is_array());
    }
}

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    fixture: String,
    mutation: Mutation,
    checks: Vec<String>,
    expected_failures: Vec<String>,
    #[serde(default)]
    fail_write: bool,
    #[serde(default)]
    fail_intent: bool,
    #[serde(default)]
    repeat_envelope: bool,
    #[serde(default)]
    load_race: bool,
    #[serde(default)]
    restart: bool,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!(
        "../../../fixtures/harness/turn-model/matrix.json"
    ))
    .unwrap()
}

fn replay(case: &Case) -> super::replay::Replay {
    let mut scenario = Scenario::recorded(&case.fixture).mutate(case.mutation);
    scenario.fail_write = case.fail_write;
    scenario.fail_intent = case.fail_intent;
    scenario.repeat_envelope = case.repeat_envelope;
    scenario.load_race = case.load_race;
    scenario.restart = case.restart;
    run(scenario)
}

fn failures(case: &Case) -> Vec<String> {
    let replay = replay(case);
    let projection = project(&replay);
    assert_eq!(replay.items, replay.reopened, "SQL reopen: {}", case.id);
    case.checks
        .iter()
        .filter(|name| !check(name, &replay, &projection))
        .cloned()
        .collect()
}

#[test]
fn known_failure_matrix_is_exact() {
    let mut mismatches = Vec::new();
    for case in cases() {
        let observed = failures(&case);
        if observed != case.expected_failures {
            mismatches.push(format!(
                "{}: observed {observed:?}; expected {:?}",
                case.id, case.expected_failures
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "unexpected failure or newly passing release gate:\n{}",
        mismatches.join("\n")
    );
}

#[test]
#[ignore = "stages 1–4: invariants 1–8; explicit expected failures in turn-model/matrix.json"]
fn release_matrix_has_no_known_failures() {
    let mut broken = Vec::new();
    for case in cases() {
        let observed = failures(&case);
        if !observed.is_empty() {
            broken.push(format!("{}: {observed:?}", case.id));
        }
    }
    for name in super::questions::NAMES {
        verify_fixture(name);
    }
    for name in super::handoff_recordings::NAMES {
        verify_fixture(name);
    }
    assert!(
        broken.is_empty(),
        "release invariants broken:\n{}",
        broken.join("\n")
    );
}

/// The matrix cases the timeline's own tests fold (`app/tests/components/harness/runRows.test.ts`).
const TIMELINE_CASES: [&str; 14] = [
    "durable-command-and-answer-reference",
    "claude-144-duplicate-a",
    "claude-144-duplicate-b",
    "answer-before-trailing-tool",
    "short-terminal-message",
    "claude-158-gap",
    "claude-160-gap",
    "claude-159-recovery",
    "claude-interrupt",
    "codex-interrupt",
    "runtime-exit-during-stream",
    "artifact-source-before-tool",
    "compaction-only-mutation",
    "restart-during-stream",
];

/// Rewrites `app/tests/fixtures/turn-model/`: each case's envelopes, its
/// reopened rows and its runs, as the webview would read them. Run by hand
/// after a bridge or settlement change.
#[test]
#[ignore = "writes fixtures into the source tree"]
fn write_timeline_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/turn-model");
    std::fs::create_dir_all(&dir).unwrap();
    let all = cases();
    for id in TIMELINE_CASES {
        let case = all
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("no case {id}"));
        let replay = replay(case);
        let json = serde_json::json!({
            "case": id,
            "restarted": replay.restarted,
            "envelopes": replay.envelopes,
            "reopened": replay.reopened,
            "runs": replay.ledger["runs"],
        });
        std::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_string_pretty(&json).unwrap() + "\n",
        )
        .unwrap();
    }
}
