use super::plist::{interval_from_plist, plist_body, program_from_plist};
use super::DEFAULT_INTERVAL_HOURS;

/// Emits the real generated plist so launchd's view of it can be checked
/// outside the app. Set OCULUS_DUMP_AGENT=<dir>; otherwise this is a no-op.
#[test]
fn dump_agent_artifacts() {
    let Some(dir) = std::env::var_os("OCULUS_DUMP_AGENT") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::write(
        dir.join("agent.plist"),
        plist_body("/usr/local/bin/oculus", 6 * 3600),
    )
    .unwrap();
}

#[test]
fn interval_round_trips_through_the_plist() {
    let dir = crate::test_support::Scratch::new("plist");
    let p = dir.join("t.plist");
    std::fs::write(&p, plist_body("/tmp/oculus", 6 * 3600)).unwrap();
    assert_eq!(interval_from_plist(&p), 6);
    std::fs::write(&p, "not a plist").unwrap();
    assert_eq!(interval_from_plist(&p), DEFAULT_INTERVAL_HOURS);
}

#[test]
fn the_program_path_round_trips_through_the_plist() {
    let dir = crate::test_support::Scratch::new("plist");
    let p = dir.join("prog.plist");
    std::fs::write(
        &p,
        plist_body("/Applications/Oculus.app/Contents/MacOS/oculus", 3600),
    )
    .unwrap();
    assert_eq!(
        program_from_plist(&p).as_deref(),
        Some("/Applications/Oculus.app/Contents/MacOS/oculus")
    );
    std::fs::write(&p, "not a plist").unwrap();
    assert_eq!(program_from_plist(&p), None);
}

/// launchd execs ProgramArguments directly — no shell — so the CLI must be
/// argv[0] with its subcommand as separate arguments, not one string.
#[test]
fn the_agent_invokes_the_cli_not_a_shell() {
    let plist = plist_body("/Applications/Oculus.app/Contents/MacOS/oculus", 6 * 3600);
    assert!(plist.contains("<string>/Applications/Oculus.app/Contents/MacOS/oculus</string>"));
    assert!(plist.contains("<string>auth</string>"));
    assert!(plist.contains("<string>tick</string>"));
    assert!(!plist.contains("/bin/sh"));
}
