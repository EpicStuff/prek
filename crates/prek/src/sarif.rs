use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::config::SarifConfig;
use crate::hook::{Hook, InstalledHook};

#[derive(Debug, Clone)]
pub(crate) enum SarifStrategy {
    NativeFlags(Vec<String>),
    Adapter { binary: String, args: Vec<String> },
}

/// Resolve SARIF strategy for a hook.
///
/// Priority:
/// 1. Hook config (`sarif`) - explicit user configuration
/// 2. `adaptors/<hook-id>.yaml`
/// 3. `adaptors/<hook-id>` binary, then `adaptors/<hook-id>.nim`
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

    resolve_adaptor_from_folder(hook)
}

#[derive(Debug, Deserialize)]
struct AdaptorYaml {
    #[serde(default)]
    flags: Vec<String>,
    binary: Option<String>,
    #[serde(default)]
    args: Vec<String>,
}

fn resolve_adaptor_from_folder(hook: &Hook) -> Result<Option<SarifStrategy>> {
    let adaptor_dir = hook.work_dir().join("adaptors");
    let yaml = adaptor_dir.join(format!("{}.yaml", hook.id));
    if yaml.is_file() {
        let content = fs_err::read_to_string(&yaml)
            .with_context(|| format!("Failed to read adaptor config `{}`", yaml.display()))?;
        let parsed: AdaptorYaml = serde_saphyr::from_str(&content)
            .with_context(|| format!("Failed to parse adaptor config `{}`", yaml.display()))?;
        return Ok(Some(strategy_from_adaptor_yaml(parsed)?));
    }

    let binary = adaptor_dir.join(&hook.id);
    if binary.is_file() {
        return Ok(Some(SarifStrategy::Adapter {
            binary: binary.to_string_lossy().to_string(),
            args: vec![],
        }));
    }

    let nim = adaptor_dir.join(format!("{}.nim", hook.id));
    if nim.is_file() {
        return Ok(Some(SarifStrategy::Adapter {
            binary: nim.to_string_lossy().to_string(),
            args: vec![],
        }));
    }

    Ok(None)
}

fn strategy_from_adaptor_yaml(adaptor: AdaptorYaml) -> Result<SarifStrategy> {
    if !adaptor.flags.is_empty() {
        return Ok(SarifStrategy::NativeFlags(adaptor.flags));
    }
    if let Some(binary) = adaptor.binary {
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

pub(crate) async fn run_adapter(binary: &str, args: &[String], input: &[u8]) -> Result<Vec<u8>> {
    let mut cmd = tokio::process::Command::new(binary);
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
    use super::{AdaptorYaml, SarifReport, SarifStrategy, strategy_from_adaptor_yaml};

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
}
