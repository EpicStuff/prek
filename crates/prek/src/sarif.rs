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
    Combined {
        flags: Vec<String>,
        adapter: Option<SarifAdapter>,
    },
    NoOutput,
}

#[derive(Debug, Clone)]
pub(crate) struct SarifAdapter {
    pub(crate) binary: String,
    pub(crate) args: Vec<String>,
}

/// Resolve SARIF strategy for a hook.
///
/// Priority:
/// 1. Hook config (`sarif`) - explicit user configuration
/// 2. Embedded adaptor metadata by hook id
pub(crate) fn resolve_strategy(hook: &Hook) -> Result<Option<SarifStrategy>> {
    if let Some(config) = &hook.sarif {
        return Ok(Some(match config {
            SarifConfig::Flags { args } => SarifStrategy::Combined {
                flags: args.clone(),
                adapter: None,
            },
            SarifConfig::Adapter { binary, args } => SarifStrategy::Combined {
                flags: vec![],
                adapter: Some(SarifAdapter {
                    binary: binary.clone(),
                    args: args.clone(),
                }),
            },
        }));
    }

    if let Some(strategy) = resolve_embedded_strategy(&hook.id)? {
        return Ok(Some(strategy));
    }

    Ok(None)
}

fn resolve_embedded_strategy(hook_id: &str) -> Result<Option<SarifStrategy>> {
    let has_embedded_binary = embedded::EMBEDDED_ADAPTOR_NAMES
        .iter()
        .any(|name| *name == hook_id);

    if let Some(yaml) = embedded::embedded_adaptor_yaml(hook_id) {
        let parsed: AdaptorYaml = serde_saphyr::from_str(yaml)
            .with_context(|| format!("Failed to parse embedded adaptor yaml for `{hook_id}`"))?;
        return Ok(Some(strategy_from_adaptor_yaml(
            parsed,
            has_embedded_binary,
            hook_id,
        )?));
    }

    if has_embedded_binary {
        return Ok(Some(SarifStrategy::Combined {
            flags: vec![],
            adapter: Some(SarifAdapter {
                binary: format!("embedded://{hook_id}"),
                args: vec![],
            }),
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

fn strategy_from_adaptor_yaml(
    adaptor: AdaptorYaml,
    has_embedded_binary: bool,
    hook_id: &str,
) -> Result<SarifStrategy> {
    let adapter = if let Some(binary) = adaptor.binary {
        Some(SarifAdapter {
            binary: normalize_adapter_binary(binary),
            args: adaptor.args,
        })
    } else if has_embedded_binary {
        Some(SarifAdapter {
            binary: format!("embedded://{hook_id}"),
            args: adaptor.args,
        })
    } else {
        None
    };

    if adaptor.flags.is_empty() && adapter.is_none() {
        return Ok(SarifStrategy::NoOutput);
    }

    Ok(SarifStrategy::Combined {
        flags: adaptor.flags,
        adapter,
    })
}

fn normalize_adapter_binary(binary: String) -> String {
    if binary.ends_with(".nim")
        && let Some(stem) = std::path::Path::new(&binary).file_stem().and_then(|s| s.to_str())
    {
        return format!("embedded://{stem}");
    }
    binary
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

pub(crate) fn with_env_var(hook: &InstalledHook, key: &str, value: &str) -> InstalledHook {
    match hook {
        InstalledHook::Installed { hook, info } => {
            let mut cloned = (**hook).clone();
            cloned.env.insert(key.to_string(), value.to_string());
            InstalledHook::Installed {
                hook: Arc::new(cloned),
                info: info.clone(),
            }
        }
        InstalledHook::NoNeedInstall(hook) => {
            let mut cloned = (**hook).clone();
            cloned.env.insert(key.to_string(), value.to_string());
            InstalledHook::NoNeedInstall(Arc::new(cloned))
        }
    }
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
#[path = "../tests/internal/sarif.rs"]
mod sarif_tests;
