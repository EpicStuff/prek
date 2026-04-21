use super::{
    AdaptorYaml, SarifReport, SarifStrategy, resolve_embedded_strategy, strategy_from_adaptor_yaml,
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
