use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
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
/// 1. Hook config (`sarif`)
/// 2. Built-in adaptor registry
pub(crate) fn resolve_strategy(hook: &Hook) -> Option<SarifStrategy> {
    if let Some(config) = &hook.sarif {
        return Some(match config {
            SarifConfig::Flags { args } => SarifStrategy::NativeFlags(args.clone()),
            SarifConfig::Adapter { binary, args } => SarifStrategy::Adapter {
                binary: binary.clone(),
                args: args.clone(),
            },
        });
    }

    builtin_strategy(hook)
}

fn builtin_strategy(hook: &Hook) -> Option<SarifStrategy> {
    let entry = hook.entry.split().ok()?;
    let cmd = Path::new(entry.first()?);
    let name = cmd
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match name.as_str() {
        "ruff" => Some(SarifStrategy::NativeFlags(vec![
            "--output-format".to_string(),
            "sarif".to_string(),
        ])),
        _ => None,
    }
}

pub(crate) fn resolve_adapter_binary(hook: &Hook, binary: &str) -> String {
    let candidate = Path::new(binary);
    if candidate.is_absolute() {
        return binary.to_string();
    }

    let adaptor_path = hook.work_dir().join("adaptors").join(binary);
    if adaptor_path.is_file() {
        return adaptor_path.to_string_lossy().to_string();
    }

    binary.to_string()
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
    use super::SarifReport;

    #[test]
    fn push_json_accepts_multiple_sarif_documents() {
        let mut report = SarifReport::default();
        let input = br#"{"runs":[{"tool":{"driver":{"name":"ruff"}},"results":[]}]}{"runs":[{"tool":{"driver":{"name":"flake8"}},"results":[]}]} "#;
        report.push_json(input).expect("should parse stream");

        let rendered = report.to_pretty_json().expect("render sarif");
        assert!(rendered.contains("\"name\": \"ruff\""));
        assert!(rendered.contains("\"name\": \"flake8\""));
    }
}
