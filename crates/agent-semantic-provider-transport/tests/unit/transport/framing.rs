use std::fs;

use crate::OutputFraming;
use crate::ProviderProcessFraming;
use crate::ProviderProcessSupervisor;

use super::support::script;
use super::support::spec;
use super::support::temp_dir;

#[tokio::test]
async fn line_framing_normalizes_line_payloads() {
    let root = temp_dir("line-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'first\\nsecond'\nprintf 'warn\\n' >&2\n",
    );
    let output = ProviderProcessSupervisor::default()
        .run_with_framing(
            spec(program, root.clone()),
            ProviderProcessFraming {
                stdout: OutputFraming::Lines,
                stderr: OutputFraming::Lines,
            },
        )
        .await
        .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"first\nsecond\n");
    assert_eq!(output.stderr.as_ref(), b"warn\n");
    assert_eq!(output.receipt.stdout_bytes(), "first\nsecond\n".len());
    assert_eq!(output.receipt.stderr_bytes(), "warn\n".len());
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn length_delimited_framing_captures_payload_bytes() {
    let root = temp_dir("length-delimited-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf '\\000\\000\\000\\005hello\\000\\000\\000\\005world'\n",
    );
    let output = ProviderProcessSupervisor::default()
        .run_with_framing(
            spec(program, root.clone()),
            ProviderProcessFraming {
                stdout: OutputFraming::LengthDelimited,
                stderr: OutputFraming::Bytes,
            },
        )
        .await
        .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"helloworld");
    assert_eq!(output.receipt.stdout_bytes(), 10);
    let _ = fs::remove_dir_all(root);
}
