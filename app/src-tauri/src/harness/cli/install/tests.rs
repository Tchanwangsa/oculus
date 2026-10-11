use super::routes::routes;
use super::*;
use crate::harness::event::Provider;
use std::sync::mpsc;

const ALL: [Provider; 4] = [
    Provider::Claude,
    Provider::Codex,
    Provider::Opencode,
    Provider::Antigravity,
];

/// The agents a package manager can install (not Antigravity).
const PACKAGED: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::Opencode];

/// `curl` is gated like every other manager, though macOS always has it.
#[test]
fn brew_only_machine_runs_the_brew_route() {
    let have = Managers {
        brew: true,
        ..Default::default()
    };
    for p in PACKAGED {
        let o = offer(p, have);
        assert!(o.runnable, "{p:?}");
        let runnable: Vec<Manager> = o
            .routes
            .iter()
            .filter(|r| r.available)
            .map(|r| r.manager)
            .collect();
        assert_eq!(runnable, vec![Manager::Brew], "{p:?}");
    }
}

#[test]
fn npm_only_machine_runs_the_npm_route() {
    let have = Managers {
        npm: true,
        ..Default::default()
    };
    for p in PACKAGED {
        let runnable: Vec<Manager> = offer(p, have)
            .routes
            .into_iter()
            .filter(|r| r.available)
            .map(|r| r.manager)
            .collect();
        assert_eq!(runnable, vec![Manager::Npm], "{p:?}");
    }
    // bun is only ever opencode's, and only when bun is there.
    let bun = Managers {
        bun: true,
        ..Default::default()
    };
    assert!(offer(Provider::Opencode, bun).runnable);
    assert!(!offer(Provider::Claude, bun).runnable);
    assert!(!offer(Provider::Codex, bun).runnable);
}

#[test]
fn antigravity_is_curl_only() {
    let managers: Vec<Manager> = routes(Provider::Antigravity)
        .iter()
        .map(|r| r.manager)
        .collect();
    assert_eq!(managers, vec![Manager::Curl]);

    let curl = Managers {
        curl: true,
        ..Default::default()
    };
    assert!(offer(Provider::Antigravity, curl).runnable);
    // Homebrew and node buy nothing here.
    for have in [
        Managers {
            brew: true,
            ..Default::default()
        },
        Managers {
            npm: true,
            ..Default::default()
        },
        Managers {
            bun: true,
            ..Default::default()
        },
    ] {
        let o = offer(Provider::Antigravity, have);
        assert!(!o.runnable);
        assert_eq!(o.routes.len(), 1, "the line is still there to copy");
    }
}

/// No manager at all still leaves every command to copy.
#[test]
fn no_manager_offers_copy_only() {
    for p in ALL {
        let o = offer(p, Managers::default());
        assert!(!o.runnable, "{p:?}");
        assert!(!o.routes.is_empty(), "{p:?}");
        assert!(o.routes.iter().all(|r| !r.available), "{p:?}");
        assert!(o.routes.iter().all(|r| !r.command.is_empty()), "{p:?}");
    }
}

#[test]
fn no_route_needs_sudo() {
    for p in ALL {
        for r in routes(p) {
            assert!(
                !r.command.split_whitespace().any(|w| w == "sudo"),
                "{p:?} {}",
                r.command
            );
        }
    }
}

/// The manager is the route id over the invoke boundary.
#[test]
fn routes_are_keyed_by_manager() {
    for p in ALL {
        let mut seen = std::collections::HashSet::new();
        for r in routes(p) {
            assert!(
                seen.insert(r.manager),
                "{p:?} has two {:?} routes",
                r.manager
            );
            assert_eq!(command_for(p, r.manager), Some(r.command));
        }
    }
    assert_eq!(command_for(Provider::Claude, Manager::Bun), None);
    assert!(start(Provider::Claude, Manager::Bun, |_| {}).is_err());
}

/// Lines stream, a non-zero exit is a failure, and `done` is last.
#[test]
fn runner_streams_then_reports_the_exit() {
    let (tx, rx) = mpsc::channel::<Line>();
    run_command(
        "echo hello; echo trouble 1>&2; exit 3",
        "installed",
        move |l| {
            let _ = tx.send(l);
        },
    )
    .unwrap();
    let lines: Vec<Line> = rx.iter().collect();
    let (last, body) = lines.split_last().expect("at least the done line");
    assert!(body.iter().all(|l| !l.done));
    let text: Vec<&str> = body.iter().filter_map(|l| l.line.as_deref()).collect();
    assert!(text.contains(&"hello"), "{text:?}");
    assert!(text.contains(&"trouble"), "{text:?}");
    assert!(last.done);
    assert_eq!(last.ok, Some(false));
    assert_eq!(last.status.as_deref(), Some("exited with status 3"));
}

#[test]
fn runner_refuses_sudo() {
    assert!(run_command("sudo make me a sandwich", "installed", |_| {}).is_err());
}
