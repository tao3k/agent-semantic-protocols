use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProviderRegisterBuild {
    pub bytes: Vec<u8>,
    pub input_paths: Vec<PathBuf>,
    pub identities: Vec<ProviderIdentityBuild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentityBuild {
    pub language_id: String,
    pub provider_id: String,
}

pub fn resolve_provider_register(
    source_root: impl AsRef<Path>,
) -> Result<ResolvedProviderRegisterBuild, String> {
    let root = source_root.as_ref();
    let index = root.join("schemas/provider-register.json");
    let register = read_json(&index)?;
    let providers = register["providers"]
        .as_array()
        .ok_or("provider register providers must be an array")?;
    let identities = providers
        .iter()
        .map(|entry| {
            Ok(ProviderIdentityBuild {
                provider_id: entry["providerId"]
                    .as_str()
                    .ok_or("provider register providerId missing")?
                    .to_owned(),
                language_id: entry["languageId"]
                    .as_str()
                    .ok_or("provider register languageId missing")?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(ResolvedProviderRegisterBuild {
        bytes: serde_json::to_vec_pretty(&register).map_err(|e| e.to_string())?,
        input_paths: vec![index],
        identities,
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(&fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embeds_identity_register_without_provider_source_paths() {
        let root = std::env::temp_dir().join(format!("asp-registry-build-{}", std::process::id()));
        fs::create_dir_all(root.join("schemas")).unwrap();
        fs::write(
            root.join("schemas/provider-register.json"),
            r#"{"providers":[{"providerId":"asp-rust","languageId":"rust"}]}"#,
        )
        .unwrap();
        let result = resolve_provider_register(&root).unwrap();
        assert_eq!(result.input_paths.len(), 1);
        assert_eq!(
            result.identities,
            [ProviderIdentityBuild {
                language_id: "rust".to_owned(),
                provider_id: "asp-rust".to_owned(),
            }]
        );
        let register: Value = serde_json::from_slice(&result.bytes).unwrap();
        assert_eq!(register["providers"][0]["providerId"], "asp-rust");
        assert!(register["providers"][0].get("descriptor").is_none());
        let _ = fs::remove_dir_all(root);
    }
}
