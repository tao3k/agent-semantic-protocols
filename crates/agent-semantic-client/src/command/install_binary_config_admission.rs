// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[cfg(feature = "mrr-data-search-composition")]
use std::path::Path;
#[cfg(feature = "mrr-data-search-composition")]
use std::path::PathBuf;

#[cfg(feature = "mrr-data-search-composition")]
pub(super) async fn admit_embedded_mrr_runtime_candidate(
    installing_asp_binary: &Path,
) -> Result<PathBuf, String> {
    let parent = installing_asp_binary
        .parent()
        .ok_or_else(|| "Runtime candidate omits artifact directory".to_owned())?;
    if parent
        .join("bundle.json")
        .try_exists()
        .map_err(|error| format!("inspect Runtime bundle: {error}"))?
    {
        let bundle =
            agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(
                parent,
            )
            .await?;
        return bundle.member_path("mrr-search").ok_or_else(|| {
            "reasonKind=mrr-search-owner-missing Runtime bundle omits MRR search owner".to_owned()
        });
    }
    resolve_executable_sibling(
        installing_asp_binary,
        &format!("mrr-native-worker{}", std::env::consts::EXE_SUFFIX),
        "MRR search owner",
    )
}

#[cfg(feature = "mrr-data-search-composition")]
fn resolve_executable_sibling(
    installing_asp_binary: &Path,
    file_name: &str,
    label: &str,
) -> Result<PathBuf, String> {
    let parent = installing_asp_binary.parent().ok_or_else(|| {
        format!(
            "installing ASP binary has no build directory: {}",
            installing_asp_binary.display()
        )
    })?;
    let candidate = parent.join(file_name);
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        format!(
            "{label} candidate is unavailable at {}: {error}; provide the matching {label} executable in the same artifact directory before installation",
            candidate.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(format!(
                "{label} candidate is not an executable regular file: {}",
                candidate.display()
            ));
        }
    }
    Ok(candidate)
}
