use std::fs;

use crate::{OutputFraming, ProviderProcessFraming, run_provider_process_with_framing};

use super::support::{script, spec, temp_dir};

#[test]
fn line_framing_normalizes_line_payloads() {
    let root = temp_dir("line-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'first\\nsecond'\nprintf 'warn\\n' >&2\n",
    );
    let output = run_provider_process_with_framing(
        spec(program, root.clone()),
        ProviderProcessFraming {
            stdout: OutputFraming::Lines,
            stderr: OutputFraming::Lines,
        },
    )
    .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"first\nsecond\n");
    assert_eq!(output.stderr.as_ref(), b"warn\n");
    assert_eq!(output.receipt.stdout_bytes, "first\nsecond\n".len());
    assert_eq!(output.receipt.stderr_bytes, "warn\n".len());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn length_delimited_framing_captures_payload_bytes() {
    let root = temp_dir("length-delimited-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf '\\000\\000\\000\\005hello\\000\\000\\000\\005world'\n",
    );
    let output = run_provider_process_with_framing(
        spec(program, root.clone()),
        ProviderProcessFraming {
            stdout: OutputFraming::LengthDelimited,
            stderr: OutputFraming::Bytes,
        },
    )
    .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"helloworld");
    assert_eq!(output.receipt.stdout_bytes, 10);
    let _ = fs::remove_dir_all(root);
}
