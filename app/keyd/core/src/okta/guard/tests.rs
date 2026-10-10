use super::*;

const APP: Role = Role::App;
const CLI: Role = Role::Cli;
const T: u64 = 1_000_000;

/// Admits `trigger` from the app at `now` and settles it as `result`.
fn attempt(r: &mut AttemptRecord, trigger: Trigger, now: u64, result: Result<String, LoginError>) {
    admit(r, trigger, APP, now).unwrap();
    settle(r, trigger, &result);
}

fn bad_totp() -> Result<String, LoginError> {
    Err(LoginError::BadTotp(String::new()))
}

fn ok() -> Result<String, LoginError> {
    Ok(String::new())
}

mod in_memory;
mod on_disk_record;
