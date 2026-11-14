use super::sweep::{alive, sweep};

#[test]
fn the_startup_sweep_clears_what_no_running_copy_owns() {
    let data = crate::test_support::Scratch::new("download-sweep");
    let root = data.join("downloads").join(".incoming");
    for folder in ["100-1", "200-4", "300-2", "stray"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(root.join(folder).join("half.zip"), b"partial").unwrap();
    }
    std::fs::write(root.join("loose.tmp"), b"x").unwrap();
    std::fs::write(data.join("downloads").join("keep"), b"not staging").unwrap();

    // 100 is us (a pid reused after a crash), 200 another running copy,
    // 300 a copy that has exited.
    sweep(&root, 100, |pid| pid == 100 || pid == 200);

    assert!(
        !root.join("100-1").exists(),
        "our own pid's folder is stale"
    );
    assert!(
        root.join("200-4").join("half.zip").exists(),
        "a live copy's"
    );
    assert!(!root.join("300-2").exists());
    assert!(!root.join("stray").exists());
    assert!(!root.join("loose.tmp").exists());
    assert!(data.join("downloads").join("keep").exists());

    sweep(&data.join("missing"), 100, |_| false);
}

#[cfg(unix)]
#[test]
fn a_running_process_is_alive_and_a_free_pid_is_not() {
    assert!(alive(std::process::id()));
    assert!(alive(std::os::unix::process::parent_id()));
    assert!(alive(1), "launchd/init belongs to root: EPERM is alive");
    assert!(!alive(0), "0 is the process group, not a process");
    assert!(!alive(u32::MAX));
}
