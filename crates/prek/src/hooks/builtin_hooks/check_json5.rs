use std::path::Path;

use crate::hook::Hook;
use crate::hooks::pre_commit_hooks::check_json::JsonValue;
use crate::hooks::run_concurrent_file_checks;
use crate::hooks::sarif_output::{HookDiagnostic, render_sarif};
use crate::run::CONCURRENCY;

pub(crate) async fn check_json5(
    hook: &Hook,
    filenames: &[&Path],
) -> anyhow::Result<(i32, Vec<u8>)> {
    run_concurrent_file_checks(filenames.iter().copied(), *CONCURRENCY, |filename| {
        check_file(hook.project().relative_path(), filename)
    })
    .await
}

pub(crate) async fn check_json5_sarif(
    hook: &Hook,
    filenames: &[&Path],
) -> anyhow::Result<(i32, Vec<u8>)> {
    let diagnostics = collect_diagnostics(hook.project().relative_path(), filenames).await?;
    if diagnostics.is_empty() {
        return Ok((0, Vec::new()));
    }
    let output = render_sarif("prek-check-json5", &hook.id, &diagnostics)?;
    Ok((1, output))
}

async fn check_file(file_base: &Path, filename: &Path) -> anyhow::Result<(i32, Vec<u8>)> {
    let diagnostics = check_file_diagnostics(file_base, filename).await?;
    if diagnostics.is_empty() {
        return Ok((0, Vec::new()));
    }

    let mut output = Vec::new();
    for diagnostic in diagnostics {
        let error_message = format!(
            "{}: Failed to json5 decode ({})\n",
            diagnostic.path, diagnostic.message
        );
        output.extend(error_message.as_bytes());
    }
    Ok((1, output))
}

async fn collect_diagnostics(
    file_base: &Path,
    filenames: &[&Path],
) -> anyhow::Result<Vec<HookDiagnostic>> {
    let mut diagnostics = Vec::new();
    for filename in filenames {
        diagnostics.extend(check_file_diagnostics(file_base, filename).await?);
    }
    Ok(diagnostics)
}

async fn check_file_diagnostics(
    file_base: &Path,
    filename: &Path,
) -> anyhow::Result<Vec<HookDiagnostic>> {
    let file_path = file_base.join(filename);
    let content = fs_err::tokio::read_to_string(file_path).await?;
    if content.is_empty() {
        return Ok(Vec::new());
    }

    match json5::from_str::<JsonValue>(&content) {
        Ok(_) => Ok(Vec::new()),
        Err(e) => {
            let (line, column) = parse_line_and_column(&e.to_string());
            let diagnostic =
                HookDiagnostic::new("prek/check-json5/parse-error", filename, e.to_string())
                    .with_location(line, column);
            Ok(vec![diagnostic])
        }
    }
}

fn parse_line_and_column(message: &str) -> (Option<u64>, Option<u64>) {
    let Some((line, column)) = message
        .lines()
        .find_map(|line| line.trim().split_once(':'))
        .and_then(|(line, column)| {
            let line = line.parse::<u64>().ok()?;
            let column = column.parse::<u64>().ok()?;
            Some((line, column))
        })
    else {
        return (None, None);
    };

    (Some(line), Some(column))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    async fn create_test_file(
        dir: &tempfile::TempDir,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<PathBuf> {
        let file_path = dir.path().join(name);
        fs_err::tokio::write(&file_path, content).await?;
        Ok(file_path)
    }

    #[tokio::test]
    async fn test_valid_json5() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let content = indoc::indoc! {r#"
        {
          // comments
          unquoted: "and you can quote me on that",
          singleQuotes: 'I can use "double quotes" here',
          lineBreaks: "Look, Mom! \
          No \\n's!",
          hexadecimal: 0xdecaf,
          leadingDecimalPoint: 0.8675309,
          andTrailing: 8675309,
          positiveSign: +1,
          trailingComma: "in objects",
          andIn: ["arrays"],
          backwardsCompatible: "with JSON",
        }
        "#};
        let file_path = create_test_file(&dir, "valid.json5", content.as_bytes()).await?;
        let (code, output) = check_file(dir.path(), &file_path).await?;
        assert_eq!(code, 0);
        assert!(output.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_keys() -> anyhow::Result<()> {
        // JSON5 warns duplicate names are unpredictable; implementations may error or accept.
        // Our JsonValue custom deserializer rejects duplicates.
        let dir = tempdir()?;
        let content = indoc::indoc! {r#"
        {
          key: "value1",
          key: "value2",
          key: "value3",
        }
        "#};
        let file_path = create_test_file(&dir, "duplicate.json5", content.as_bytes()).await?;
        let (code, output) = check_file(dir.path(), &file_path).await?;
        assert_eq!(code, 1);
        assert!(String::from_utf8_lossy(&output).contains("duplicate key"));

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_json5() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let file_path = create_test_file(&dir, "invalid.json5", b"{ key: 'value' ").await?;
        let (code, output) = check_file(dir.path(), &file_path).await?;
        assert_eq!(code, 1);
        assert!(!output.is_empty());

        Ok(())
    }
}
