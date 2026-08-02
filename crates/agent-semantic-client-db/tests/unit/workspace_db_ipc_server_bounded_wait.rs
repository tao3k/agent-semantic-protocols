#[tokio::test]
async fn pending_generation_wait_terminates_at_its_bound() {
    let result = super::bounded_runtime_generation_wait(
        std::time::Duration::from_millis(1),
        std::future::pending::<Result<(), String>>(),
        "generation wait timed out".to_owned(),
    )
    .await;

    assert_eq!(result, Err("generation wait timed out".to_owned()));
}
