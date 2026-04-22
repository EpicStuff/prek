use super::{
    AdaptorYaml, SarifReport, SarifStrategy, render_builtin_sarif_run, resolve_embedded_strategy,
    strategy_from_adaptor_yaml,
};
use std::path::Path;

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
    let strategy = strategy_from_adaptor_yaml(
        AdaptorYaml {
            flags: vec!["--output-format".to_string(), "sarif".to_string()],
            binary: None,
            args: vec![],
        },
        false,
        "ruff-check",
    )
    .expect("flags strategy should parse");
    match strategy {
        SarifStrategy::Combined {
            flags,
            adapter: None,
        } => {
            assert_eq!(flags, vec!["--output-format", "sarif"]);
        }
        _ => panic!("expected native flags strategy"),
    }
}

#[test]
fn adaptor_yaml_binary_strategy() {
    let strategy = strategy_from_adaptor_yaml(
        AdaptorYaml {
            flags: vec![],
            binary: Some("adaptors/ruff-check".to_string()),
            args: vec!["--foo".to_string()],
        },
        false,
        "ruff-check",
    )
    .expect("binary strategy should parse");
    match strategy {
        SarifStrategy::Combined {
            flags,
            adapter: Some(adapter),
        } => {
            assert!(flags.is_empty());
            let binary = adapter.binary;
            let args = adapter.args;
            assert_eq!(binary, "adaptors/ruff-check");
            assert_eq!(args, vec!["--foo"]);
        }
        _ => panic!("expected adapter strategy"),
    }
}

#[test]
fn adaptor_yaml_flags_and_implicit_embedded_binary_can_be_combined() {
    let strategy = strategy_from_adaptor_yaml(
        AdaptorYaml {
            flags: vec!["--output-format".to_string(), "sarif".to_string()],
            binary: None,
            args: vec![],
        },
        true,
        "basedpyright",
    )
    .expect("strategy should parse");

    assert!(matches!(
        strategy,
        SarifStrategy::Combined { flags, adapter: Some(adapter) }
            if flags == vec!["--output-format", "sarif"]
                && adapter.binary == "embedded://basedpyright"
                && adapter.args.is_empty()
    ));
}

#[test]
fn adaptor_yaml_rejects_empty_strategy() {
    let err = strategy_from_adaptor_yaml(
        AdaptorYaml {
            flags: vec![],
            binary: None,
            args: vec![],
        },
        false,
        "basedpyright",
    )
    .expect_err("empty adaptor should fail");

    assert!(
        err.to_string()
            .contains("Adaptor YAML must specify either `flags` or `binary`")
    );
}

#[test]
fn embedded_ruff_check_uses_native_flags() {
    let strategy = resolve_embedded_strategy("ruff-check")
        .expect("strategy resolution should succeed")
        .expect("strategy should exist");
    match strategy {
        SarifStrategy::Combined {
            flags,
            adapter: None,
        } => {
            assert_eq!(flags, vec!["--output-format", "sarif"]);
        }
        _ => panic!("expected native flags"),
    }
}

#[test]
fn embedded_basedpyright_uses_flags_and_implicit_adapter() {
    let strategy = resolve_embedded_strategy("basedpyright")
        .expect("strategy resolution should succeed")
        .expect("strategy should exist");
    match strategy {
        SarifStrategy::Combined {
            flags,
            adapter: Some(adapter),
        } => {
            assert_eq!(flags, vec!["--outputjson"]);
            assert_eq!(adapter.binary, "embedded://basedpyright");
        }
        other => panic!("unexpected resolution result: {other:?}"),
    }
}

#[test]
fn builtin_output_renders_valid_sarif_json() {
    let file_refs = [Path::new("src/main.rs"), Path::new("README.md")];
    let output = b"Fixing src/main.rs\n".to_vec();
    let rendered =
        render_builtin_sarif_run("trailing-whitespace", &file_refs, 1, &output).expect("sarif");
    let value: serde_json::Value = serde_json::from_slice(&rendered).expect("valid json");
    assert_eq!(
        value["tool"]["driver"]["name"].as_str(),
        Some("trailing-whitespace")
    );
    assert_eq!(
        value["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"].as_str(),
        Some("src/main.rs")
    );
}
