#[cfg(unix)]
mod unix {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::process::Output;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    const MANIFEST: &[u8] =
        include_bytes!("../../../../asp-codex-plugin/.codex-plugin/plugin.json");
    const HOOKS: &[u8] = include_bytes!("../../../../asp-codex-plugin/hooks/hooks.json");
    const LAUNCHER: &[u8] = include_bytes!("../../../../asp-codex-plugin/bin/asp-hook-exec");

    #[test]
    fn plugin_status_defaults_source_to_global_state_home_dev_root() {
        let fixture = Fixture::new("plugin-status-global-default");
        fixture.write_dev_root();
        let before = fixture.payload_digest();
        let output = fixture.run(&["status", "--codex"]);
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("pluginScope=global"), "{stdout}");
        assert!(
            stdout.contains("sourceRootSource=state-home-dev"),
            "{stdout}"
        );
        assert!(stdout.contains("state=plugin-cache-missing"), "{stdout}");
        assert!(stdout.contains("publicationStatus=observed"), "{stdout}");
        assert_eq!(fixture.payload_digest(), before);
        assert_eq!(fixture.add_count(), 0);
    }

    #[test]
    fn plugin_publish_uses_global_cache_and_is_idempotent() {
        let fixture = Fixture::new("plugin-publish-global-idempotent");
        let root = fixture.root.to_str().expect("fixture root");
        let first = fixture.run(&["publish", "--codex", root]);
        assert_success(&first);
        let stdout = String::from_utf8_lossy(&first.stdout);
        for fact in [
            "pluginScope=global",
            "sourceRootSource=explicit-override",
            "publicationStatus=updated",
            "binaryInstall=not-on-plugin-publication",
            "hookGeneration=not-on-plugin-publication",
            "runtimeActivation=not-on-plugin-publication",
        ] {
            assert!(stdout.contains(fact), "missing {fact}: {stdout}");
        }
        assert_eq!(fixture.add_count(), 1);
        assert!(!fixture.root.join(".codex/plugins").exists());

        let second = fixture.run(&["publish", "--codex", root]);
        assert_success(&second);
        assert!(String::from_utf8_lossy(&second.stdout).contains("publicationStatus=unchanged"));
        assert_eq!(fixture.add_count(), 1);
    }

    #[test]
    fn plugin_publish_falls_back_to_the_deterministic_global_cache_without_codex_cli() {
        let fixture = Fixture::new("plugin-publish-direct-global-cache");
        let root = fixture.root.to_str().expect("fixture root");
        let output = fixture.run_without_codex(&["publish", "--codex", root]);
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("publicationTransport=direct-global-cache"),
            "{stdout}"
        );
        assert!(
            stdout.contains("hookTrust=codex-review-required"),
            "{stdout}"
        );
        let version = fixture.payload_version();
        assert_eq!(
            fixture.installed_payload_digest(&version),
            fixture.payload_digest()
        );
        assert_eq!(fixture.add_count(), 0);
        assert!(
            fixture
                .installed_payload_root(&version)
                .join("bin/asp-hook-exec")
                .metadata()
                .expect("published Hook launcher metadata")
                .permissions()
                .mode()
                & 0o111
                != 0,
            "direct global cache publication must preserve the launcher executable bit; otherwise Codex only renders `hook exited with code 126`"
        );

        let launcher = fixture
            .installed_payload_root(&version)
            .join("bin/asp-hook-exec");
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o600))
            .expect("remove launcher executable bit");
        let repaired = fixture.run_without_codex(&["publish", "--codex", root]);
        assert_success(&repaired);
        assert!(
            String::from_utf8_lossy(&repaired.stdout)
                .contains("publicationStatus=repaired-launcher-mode"),
            "{}",
            String::from_utf8_lossy(&repaired.stdout)
        );
        assert_ne!(
            launcher
                .metadata()
                .expect("repaired launcher metadata")
                .permissions()
                .mode()
                & 0o111,
            0,
            "mode-only cache repair must remove the Codex exit-126 condition"
        );
    }

    #[test]
    fn failed_publication_restores_source_and_previous_global_cache() {
        let fixture = Fixture::new("plugin-publish-rollback");
        let root = fixture.root.to_str().expect("fixture root");
        let initial = fixture.run(&["publish", "--codex", root]);
        assert_success(&initial);
        let installed_version = fixture.installed_version();
        let installed_before = fixture.installed_payload_digest(&installed_version);

        let hooks_path = fixture.plugin_root.join("hooks/hooks.json");
        let mut changed_hooks = std::fs::read(&hooks_path).expect("read hooks");
        changed_hooks.push(b'\n');
        std::fs::write(&hooks_path, &changed_hooks).expect("change hooks");
        let manifest_path = fixture.plugin_root.join(".codex-plugin/plugin.json");
        let manifest_before = std::fs::read(&manifest_path).expect("read manifest");
        std::fs::write(fixture.codex_home.join("fail-next-add"), b"1")
            .expect("arm one-shot failure");

        let failed = fixture.run(&["publish", "--codex", root]);
        assert!(!failed.status.success());
        assert!(
            String::from_utf8_lossy(&failed.stderr)
                .contains("reasonKind=plugin-payload-publication-failed")
        );
        assert_eq!(
            std::fs::read(&manifest_path).expect("restored manifest"),
            manifest_before
        );
        assert_eq!(
            std::fs::read(&hooks_path).expect("restored hooks"),
            changed_hooks
        );
        assert_eq!(fixture.installed_version(), installed_version);
        assert_eq!(
            fixture.installed_payload_digest(&installed_version),
            installed_before
        );
    }

    #[test]
    fn plugin_help_declares_global_default_and_low_priority_source_override() {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(["install", "plugin", "status", "--help"])
            .output()
            .expect("plugin status help");
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        for fact in [
            "asp install plugin status",
            "[PROJECT_ROOT]",
            "publication remains global",
            "ASP_STATE_HOME [dev].root",
        ] {
            assert!(stdout.contains(fact), "missing {fact}: {stdout}");
        }
        assert!(!stdout.contains("--project"), "{stdout}");
        assert!(!stdout.contains("--global"), "{stdout}");
    }

    struct Fixture {
        root: PathBuf,
        plugin_root: PathBuf,
        codex_home: PathBuf,
        state_home: PathBuf,
        fake_bin: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = temp_root(name);
            let plugin_root = root.join("asp-codex-plugin");
            let codex_home = root.join("global-codex-home");
            let state_home = root.join("global-asp-state-home");
            std::fs::create_dir_all(&codex_home).expect("create Codex home");
            std::fs::create_dir_all(&state_home).expect("create ASP state home");
            write_bundle(&plugin_root, MANIFEST, HOOKS, LAUNCHER);
            let fake_bin = write_fake_codex(&root);
            Self {
                root,
                plugin_root,
                codex_home,
                state_home,
                fake_bin,
            }
        }

        fn write_dev_root(&self) {
            std::fs::write(
                self.state_home.join("asp.toml"),
                format!("[dev]\nenabled = true\nroot = {:?}\n", self.root),
            )
            .expect("write dev root");
        }

        fn run(&self, args: &[&str]) -> Output {
            let mut path = vec![self.fake_bin.clone()];
            path.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            Command::new(env!("CARGO_BIN_EXE_asp"))
                .current_dir(&self.root)
                .env("CODEX_HOME", &self.codex_home)
                .env("ASP_STATE_HOME", &self.state_home)
                .env("PATH", std::env::join_paths(path).expect("join PATH"))
                .arg("install")
                .arg("plugin")
                .args(args)
                .output()
                .expect("run plugin command")
        }

        fn run_without_codex(&self, args: &[&str]) -> Output {
            Command::new(env!("CARGO_BIN_EXE_asp"))
                .current_dir(&self.root)
                .env("CODEX_HOME", &self.codex_home)
                .env("ASP_STATE_HOME", &self.state_home)
                .env("PATH", "/usr/bin:/bin")
                .arg("install")
                .arg("plugin")
                .args(args)
                .output()
                .expect("run plugin command without Codex CLI")
        }

        fn payload_digest(&self) -> String {
            agent_semantic_config::load_codex_plugin_payload_identity(&self.plugin_root)
                .expect("payload identity")
                .digest
        }

        fn payload_version(&self) -> String {
            agent_semantic_config::load_codex_plugin_payload_identity(&self.plugin_root)
                .expect("payload identity")
                .version
        }

        fn installed_version(&self) -> String {
            std::fs::read_to_string(self.codex_home.join("installed-version"))
                .expect("installed version")
                .trim()
                .to_owned()
        }

        fn installed_payload_digest(&self, version: &str) -> String {
            let root = self.installed_payload_root(version);
            agent_semantic_config::load_codex_plugin_payload_identity(&root)
                .expect("installed payload identity")
                .digest
        }

        fn installed_payload_root(&self, version: &str) -> PathBuf {
            self.codex_home
                .join("plugins/cache/asp-project/asp-codex-plugin")
                .join(version)
        }

        fn add_count(&self) -> u64 {
            std::fs::read_to_string(self.codex_home.join("add-count"))
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(0)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).expect("cleanup plugin fixture");
        }
    }

    fn write_bundle(root: &Path, manifest: &[u8], hooks: &[u8], launcher: &[u8]) {
        for (relative, bytes) in [
            (".codex-plugin/plugin.json", manifest),
            ("hooks/hooks.json", hooks),
            ("bin/asp-hook-exec", launcher),
        ] {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().expect("payload parent"))
                .expect("create payload parent");
            std::fs::write(path, bytes).expect("write payload");
            if relative == "bin/asp-hook-exec" {
                std::fs::set_permissions(
                    root.join(relative),
                    std::fs::Permissions::from_mode(0o500),
                )
                .expect("make source Hook launcher executable");
            }
        }
    }

    fn write_fake_codex(root: &Path) -> PathBuf {
        let bin = root.join("fake-bin");
        std::fs::create_dir_all(&bin).expect("create fake bin");
        let script = bin.join("codex");
        std::fs::write(&script, r#"#!/bin/sh
set -eu
if [ "$1 $2 $3" = "plugin marketplace list" ]; then
  printf '{"marketplaces":[]}\n'; exit 0
fi
if [ "$1 $2 $3" = "plugin marketplace add" ]; then
  printf '{"status":"added"}\n'; exit 0
fi
if [ "$1 $2" = "plugin list" ]; then
  if [ -f "$CODEX_HOME/installed-version" ]; then
    version=$(sed -n '1p' "$CODEX_HOME/installed-version")
    printf '{"installed":[{"pluginId":"asp-codex-plugin@asp-project","version":"%s"}]}\n' "$version"
  else
    printf '{"installed":[]}\n'
  fi
  exit 0
fi
if [ "$1 $2" = "plugin add" ]; then
  if [ -f "$CODEX_HOME/fail-next-add" ]; then
    rm "$CODEX_HOME/fail-next-add"; printf 'injected add failure\n' >&2; exit 42
  fi
  source_root="$PWD/asp-codex-plugin"
  version=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$source_root/.codex-plugin/plugin.json" | head -n 1)
  installed="$CODEX_HOME/plugins/cache/asp-project/asp-codex-plugin/$version"
  mkdir -p "$installed/.codex-plugin" "$installed/hooks" "$installed/bin"
  cp "$source_root/.codex-plugin/plugin.json" "$installed/.codex-plugin/plugin.json"
  cp "$source_root/hooks/hooks.json" "$installed/hooks/hooks.json"
  cp "$source_root/bin/asp-hook-exec" "$installed/bin/asp-hook-exec"
  printf '%s\n' "$version" > "$CODEX_HOME/installed-version"
  count=0
  if [ -f "$CODEX_HOME/add-count" ]; then count=$(sed -n '1p' "$CODEX_HOME/add-count"); fi
  count=$((count + 1)); printf '%s\n' "$count" > "$CODEX_HOME/add-count"
  printf '{"installedPath":"%s"}\n' "$installed"; exit 0
fi
printf 'unsupported fake codex argv: %s\n' "$*" >&2; exit 64
"#).expect("write fake codex");
        let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod fake codex");
        bin
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("asp-{name}-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create fixture root");
        root
    }

    fn assert_success(output: &Output) {
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
