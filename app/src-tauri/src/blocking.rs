//! Run synchronous commands off the async runtime, preserving join failures.

pub(crate) async fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work).await.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn work_errors_and_panics_return_to_the_command() {
        assert_eq!(run::<()>(|| Err("work failed".into())).await.unwrap_err(), "work failed");
        let error = run::<()>(|| panic!("worker panic")).await.unwrap_err();
        assert!(error.contains("worker panic"), "{error}");
    }
}
