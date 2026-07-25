use super::install_command;

#[test]
fn install_language_from_workspace_is_clap_owned() {
    install_command()
        .try_get_matches_from(["install", "language", "rust", ".", "--from-workspace"])
        .expect("clap must own the workspace provider install surface");
}

#[test]
fn install_language_receipt_reconciliation_is_clap_owned_and_exclusive() {
    install_command()
        .try_get_matches_from(["install", "language", "rust", ".", "--reconcile-receipt"])
        .expect("clap must own provider receipt reconciliation");
    install_command()
        .try_get_matches_from([
            "install",
            "language",
            "rust",
            ".",
            "--reconcile-receipt",
            "--from-workspace",
        ])
        .expect_err("provider reconciliation and workspace build must be exclusive");
}
