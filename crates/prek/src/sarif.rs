use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::config::SarifConfig;
use crate::hook::{Hook, InstalledHook};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_adaptors.rs"));
}

#[derive(Debug, Clone)]
pub(crate) enum SarifStrategy {
    NativeFlags(Vec<String>),
    Adapter { binary: String, args: Vec<String> },
}

/// Resolve SARIF strategy for a hook.
///
/// Priority:
/// 1. Hook config (`sarif`) - explicit user configuration
/// 2. Embedded adaptor metadata by hook id
pub(crate) fn resolve_strategy(hook: &Hook) -> Result<Option<SarifStrategy>> {
    if let Some(config) = &hook.sarif {
        return Ok(Some(match config {
            SarifConfig::Flags { args } => SarifStrategy::NativeFlags(args.clone()),
            SarifConfig::Adapter { binary, args } => SarifStrategy::Adapter {
                binary: binary.clone(),
                args: args.clone(),
            },
        }));
    }

    if let Some(strategy) = resolve_embedded_strategy(&hook.id)? {
        return Ok(Some(strategy));
    }

    Ok(None)
}

fn resolve_embedded_strategy(hook_id: &str) -> Result<Option<SarifStrategy>> {
    if let Some(yaml) = embedded::embedded_adaptor_yaml(hook_id) {
        let parsed: AdaptorYaml = serde_saphyr::from_str(yaml)
            .with_context(|| format!("Failed to parse embedded adaptor yaml for `{hook_id}`"))?;
        return Ok(Some(strategy_from_adaptor_yaml(parsed)?));
    }

    if embedded::EMBEDDED_ADAPTOR_NAMES
        .iter()
        .any(|name| *name == hook_id)
    {
        return Ok(Some(SarifStrategy::Adapter {
            binary: format!("embedded://{hook_id}"),
            args: vec![],
        }));
    }

    Ok(None)
}

#[derive(Debug, Deserialize)]
struct AdaptorYaml {
    #[serde(default)]
    flags: Vec<String>,
    binary: Option<String>,
    #[serde(default)]
    args: Vec<String>,
}

fn strategy_from_adaptor_yaml(adaptor: AdaptorYaml) -> Result<SarifStrategy> {
    if !adaptor.flags.is_empty() {
        return Ok(SarifStrategy::NativeFlags(adaptor.flags));
    }
    if let Some(binary) = adaptor.binary {
        if binary.ends_with(".nim")
            && let Some(stem) = std::path::Path::new(&binary).file_stem().and_then(|s| s.to_str())
        {
            return Ok(SarifStrategy::Adapter {
                binary: format!("embedded://{stem}"),
                args: adaptor.args,
            });
        }
        return Ok(SarifStrategy::Adapter {
            binary,
            args: adaptor.args,
        });
    }
    anyhow::bail!("Adaptor YAML must specify either `flags` or `binary`")
}

pub(crate) fn with_native_flags(hook: &InstalledHook, flags: &[String]) -> InstalledHook {
    match hook {
        InstalledHook::Installed { hook, info } => {
            let mut cloned = (**hook).clone();
            cloned.args.extend(flags.iter().cloned());
            InstalledHook::Installed {
                hook: Arc::new(cloned),
                info: info.clone(),
            }
        }
        InstalledHook::NoNeedInstall(hook) => {
            let mut cloned = (**hook).clone();
            cloned.args.extend(flags.iter().cloned());
            InstalledHook::NoNeedInstall(Arc::new(cloned))
        }
    }
}

/// Split output into a leading non-JSON preamble and probable JSON payload.
///
/// Some tools print warnings to stderr before emitting SARIF JSON. If a hook
/// runner merges streams, this allows forwarding the preamble while still
/// parsing SARIF.
pub(crate) fn split_leading_non_json(bytes: &[u8]) -> (&[u8], &[u8]) {
    let Some(first_non_ws) = bytes.iter().position(|b| !b.is_ascii_whitespace()) else {
        return (&[], bytes);
    };

    if matches!(bytes[first_non_ws], b'{' | b'[') {
        return (&[], bytes);
    }

    for idx in first_non_ws..bytes.len() {
        if matches!(bytes[idx], b'{' | b'[')
            && (idx == 0 || matches!(bytes[idx - 1], b'\n' | b'\r'))
        {
            return (&bytes[..idx], &bytes[idx..]);
        }
    }

    (&[], bytes)
}

pub(crate) async fn run_adapter(binary: &str, args: &[String], input: &[u8]) -> Result<Vec<u8>> {
    let binary = materialize_embedded_adaptor(binary)?;
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).await?;
    }
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "SARIF adapter `{binary}` failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn materialize_embedded_adaptor(binary: &str) -> Result<String> {
    let Some(name) = binary.strip_prefix("embedded://") else {
        return Ok(binary.to_string());
    };

    let (file_name, bytes) = embedded::embedded_adaptor(name)
        .with_context(|| format!("Embedded adaptor `{name}` was not found in this build"))?;

    let dir = std::env::temp_dir().join("prek-adaptors");
    fs_err::create_dir_all(&dir).context("Failed to create temporary adaptor directory")?;
    let path = dir.join(file_name);
    if !path.exists() {
        fs_err::write(&path, bytes).with_context(|| {
            format!(
                "Failed to write embedded adaptor `{name}` to temporary path `{}`",
                path.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs_err::metadata(&path)?.permissions();
            perms.set_mode(0o755);
            fs_err::set_permissions(&path, perms)?;
        }
    }
    Ok(path.to_string_lossy().to_string())
}

#[derive(Debug, Serialize)]
pub(crate) struct SarifReport {
    version: &'static str,
    #[serde(rename = "$schema")]
    schema: &'static str,
    runs: Vec<Value>,
}

impl Default for SarifReport {
    fn default() -> Self {
        Self {
            version: "2.1.0",
            schema: "https://json.schemastore.org/sarif-2.1.0.json",
            runs: Vec::new(),
        }
    }
}

impl SarifReport {
    pub(crate) fn push_json(&mut self, bytes: &[u8]) -> Result<()> {
        let mut found = false;
        let stream = serde_json::Deserializer::from_slice(bytes).into_iter::<Value>();
        for value in stream {
            let value = value?;
            if let Some(runs) = value.get("runs").and_then(Value::as_array) {
                self.runs.extend(runs.iter().cloned());
                found = true;
                continue;
            }
            if value.get("tool").is_some() {
                self.runs.push(value);
                found = true;
                continue;
            }
            anyhow::bail!("Output is not SARIF: missing `runs` array");
        }
        if !found && !bytes.trim_ascii().is_empty() {
            anyhow::bail!("Output is not SARIF: missing `runs` array");
        }
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    pub(crate) fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdaptorYaml, SarifReport, SarifStrategy, resolve_embedded_strategy, split_leading_non_json,
        strategy_from_adaptor_yaml,
    };

    #[test]
    fn push_json_accepts_multiple_sarif_documents() {
        let mut report = SarifReport::default();
        let input = br#"{"runs":[{"tool":{"driver":{"name":"ruff"}},"results":[]}]}{"runs":[{"tool":{"driver":{"name":"flake8"}},"results":[]}]} "#;
        report.push_json(input).expect("should parse stream");

        let rendered = report.to_pretty_json().expect("render sarif");
        assert!(rendered.contains("\"name\": \"ruff\""));
        assert!(rendered.contains("\"name\": \"flake8\""));
    }

    #[test]
    fn adaptor_yaml_flags_strategy() {
        let strategy = strategy_from_adaptor_yaml(AdaptorYaml {
            flags: vec!["--output-format".to_string(), "sarif".to_string()],
            binary: None,
            args: vec![],
        })
        .expect("flags strategy should parse");
        match strategy {
            SarifStrategy::NativeFlags(flags) => {
                assert_eq!(flags, vec!["--output-format", "sarif"]);
            }
            _ => panic!("expected native flags strategy"),
        }
    }

    #[test]
    fn adaptor_yaml_binary_strategy() {
        let strategy = strategy_from_adaptor_yaml(AdaptorYaml {
            flags: vec![],
            binary: Some("adaptors/ruff-check".to_string()),
            args: vec!["--foo".to_string()],
        })
        .expect("binary strategy should parse");
        match strategy {
            SarifStrategy::Adapter { binary, args } => {
                assert_eq!(binary, "adaptors/ruff-check");
                assert_eq!(args, vec!["--foo"]);
            }
            _ => panic!("expected adapter strategy"),
        }
    }

    #[test]
    fn embedded_ruff_check_uses_native_flags() {
        let strategy = resolve_embedded_strategy("ruff-check")
            .expect("strategy resolution should succeed")
            .expect("strategy should exist");
        match strategy {
            SarifStrategy::NativeFlags(flags) => {
                assert_eq!(flags, vec!["--output-format", "sarif"]);
            }
            _ => panic!("expected native flags"),
        }
    }

    #[test]
    fn split_leading_non_json_splits_warning_preamble() {
        let output = br#"warning: ignored option
{"runs":[{"tool":{"driver":{"name":"ruff"}}}]}"#;
        let (preamble, payload) = split_leading_non_json(output);

        assert_eq!(preamble, b"warning: ignored option\n");
        assert_eq!(payload, br#"{"runs":[{"tool":{"driver":{"name":"ruff"}}}]}"#);
    }

    #[test]
    fn split_leading_non_json_leaves_json_only_output_intact() {
        let output = br#"{"runs":[{"tool":{"driver":{"name":"ruff"}}}]}"#;
        let (preamble, payload) = split_leading_non_json(output);

        assert!(preamble.is_empty());
        assert_eq!(payload, output);
    }
}
