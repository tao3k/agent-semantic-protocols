// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::PathBuf;

use agent_semantic_provider_protocol::canonical_provider_core_schemas;

fn main() -> Result<(), String> {
    let schemas_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let generated = canonical_provider_core_schemas()?;
    for (filename, value) in generated {
        let mut encoded = serde_json::to_string_pretty(&value)
            .map_err(|err| format!("serialize {filename}: {err}"))?;
        encoded.push('\n');
        let target = schemas_dir.join(filename);
        fs::write(&target, encoded)
            .map_err(|err| format!("write {}: {err}", target.display()))?;
        println!("emitted {}", target.display());
    }
    Ok(())
}
