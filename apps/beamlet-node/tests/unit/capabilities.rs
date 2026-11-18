use super::*;

/// 验证两种内置能力可以同时启用.
#[test]
fn accepts_builtin_capabilities() {
    validate(
        &registered().unwrap(),
        &["text.reverse/v1".into(), "text.uppercase/v1".into()].into(),
    )
    .unwrap();
}

/// 验证空能力集合, 未知标识及不支持的版本被拒绝.
#[test]
fn rejects_empty_or_unknown_capabilities() {
    let builtins = registered().unwrap();
    assert!(validate(&builtins, &BTreeSet::new()).is_err());
    for id in [
        "",
        "text.reverse",
        "text.reverse/v2",
        "TEXT.REVERSE/v1",
        "text.reverse/v1 ",
        "text.uppercase/v2",
        "unknown/v1",
    ] {
        assert!(validate(&builtins, &[id.into()].into()).is_err(), "{id:?}");
    }
}
