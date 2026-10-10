use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::harness::{call_at, Keyd, DEADLINE};

/// Under `dev`, the role comes from the caller's file name, so the Okta ops
/// run in a copy of this test binary named `oculus`. This is that copy's
/// half: it does nothing unless the parent test set the endpoint.
#[test]
fn okta_calls_made_as_the_cli() {
    use keyd_core::test_support::okta_fake::{PASSWORD, SEED, USERNAME};
    let Some(sock) = std::env::var_os("KEYD_TEST_CLI_SOCK") else {
        return;
    };
    let sock = PathBuf::from(sock);
    let replies = vec![
        call_at(&sock, &json!({"op": "okta_status"})),
        call_at(
            &sock,
            &json!({"op": "okta_save", "username": " ", "password": PASSWORD, "totp_secret": SEED}),
        ),
        call_at(
            &sock,
            &json!({"op": "okta_save", "username": USERNAME, "password": PASSWORD, "totp_secret": SEED}),
        ),
        call_at(&sock, &json!({"op": "okta_status"})),
        call_at(
            &sock,
            &json!({"op": "ensure_signed_in", "trigger": "manual"}),
        ),
        call_at(
            &sock,
            &json!({"op": "ensure_signed_in", "trigger": "browser"}),
        ),
        call_at(&sock, &json!({"op": "okta_forget"})),
        call_at(&sock, &json!({"op": "okta_status"})),
        call_at(&sock, &json!({"op": "session_status"})),
    ];
    println!("REPLIES {}", Value::Array(replies));
}

#[test]
fn the_binary_saves_credentials_and_signs_in_for_the_cli_only() {
    if !cfg!(feature = "dev") {
        return;
    }
    use keyd_core::test_support::okta_fake::{
        code_from_t0, script, COOKIE, PASSWORD, SEED, T0, USERNAME,
    };
    use keyd_core::test_support::FakeOrigin;

    let fake = FakeOrigin::start(script(code_from_t0));
    let now = T0.to_string();
    let port = fake.origin.rsplit(':').next().unwrap().to_string();
    let mut keyd = Keyd::start_by_name(
        60,
        &[
            ("OCULUS_KEYD_NOW", &now),
            (
                "OCULUS_KEYD_CANVAS_ORIGIN",
                &format!("http://127.0.0.1:{port}"),
            ),
            (
                "OCULUS_KEYD_SSO_ORIGIN",
                &format!("http://localhost:{port}"),
            ),
        ],
    );

    // This process is neither the app nor the CLI.
    let refused = keyd.call(json!({"op": "okta_status"}));
    assert_eq!(refused["error"], "caller", "{refused}");
    let refused = keyd.call(json!({"op": "ensure_signed_in", "trigger": "manual"}));
    assert_eq!(refused["error"], "caller", "{refused}");
    assert!(fake.hits().is_empty());

    let cli = keyd.dir.join("oculus");
    std::fs::copy(std::env::current_exe().unwrap(), &cli).unwrap();
    // Output goes to files, so a full pipe cannot stall the child while this
    // side polls for its exit.
    let (out_path, err_path) = (keyd.dir.join("cli.out"), keyd.dir.join("cli.err"));
    let mut cli_child = Command::new(&cli)
        .args([
            "okta::okta_calls_made_as_the_cli",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("KEYD_TEST_CLI_SOCK", &keyd.sock)
        .stdout(std::fs::File::create(&out_path).unwrap())
        .stderr(std::fs::File::create(&err_path).unwrap())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while cli_child.try_wait().unwrap().is_none() {
        if started.elapsed() > DEADLINE {
            cli_child.kill().ok();
            cli_child.wait().ok();
            panic!(
                "the CLI copy did not finish in {DEADLINE:?}: {}",
                std::fs::read_to_string(&err_path).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();
    let replies: Vec<Value> = stdout
        .lines()
        .find_map(|l| l.split_once("REPLIES ").map(|(_, json)| json))
        .map(|l| serde_json::from_str(l).unwrap())
        .unwrap_or_else(|| {
            panic!(
                "{stdout}\n{}",
                std::fs::read_to_string(&err_path).unwrap_or_default()
            )
        });

    assert_eq!(
        replies[0],
        json!({"username": null, "has_password": false, "has_totp": false})
    );
    assert_eq!(
        replies[1],
        json!({"error": "request", "detail": "Username is required."})
    );
    assert_eq!(replies[2], json!({"saved": true}));
    assert_eq!(
        replies[3],
        json!({"username": USERNAME, "has_password": true, "has_totp": true})
    );
    assert_eq!(replies[4], json!({"result": "signed_in"}));
    assert_eq!(replies[5]["code"], "waiting", "{}", replies[5]);
    assert_eq!(replies[6], json!({"existed": true, "legacy": "absent"}));
    assert_eq!(
        replies[7],
        json!({"username": null, "has_password": false, "has_totp": false})
    );

    // The sign-in's sessions are in the vault, not in files.
    assert_eq!(
        replies[8],
        json!({"canvas": true, "sso": true, "ed": false, "authenticated": true, "signed_out": false})
    );
    assert!(!keyd.dir.join("canvas-session.cookie").exists());
    assert!(!keyd.dir.join("sso-session.cookie").exists());
    let vault = std::fs::read(keyd.dir.join("vault.bin")).unwrap();
    for secret in [PASSWORD, COOKIE, "sid=sess1"] {
        assert!(
            !vault.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "{secret} in the vault file in clear"
        );
    }

    keyd.child.kill().ok();
    keyd.child.wait().ok();
    let mut log = String::new();
    std::io::Read::read_to_string(keyd.child.stderr.as_mut().unwrap(), &mut log).unwrap();
    for needed in [
        "op=okta_save",
        "op=ensure_signed_in",
        "result=signed_in",
        "op=okta_status",
    ] {
        assert!(log.contains(needed), "{needed} missing from {log}");
    }
    for leak in [PASSWORD, SEED, COOKIE, "Password is incorrect"] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }

    // The sessions outlive this keyd, and the app, alone, can read them back.
    keyd.restart_as("app");
    let cookies = keyd.call(json!({"op": "session_get"}));
    assert_eq!(
        cookies,
        json!({"canvas": COOKIE, "sso": "JSESSIONID=js1; sid=sess1"})
    );
    keyd.restart_as("cli");
    let refused = keyd.call(json!({"op": "session_get"}));
    assert_eq!(refused["error"], "caller", "{refused}");
}
