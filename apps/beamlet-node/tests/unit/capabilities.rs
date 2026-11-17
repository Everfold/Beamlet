use super::*;

/// 验证大写能力保留空白, 正确扩展 Unicode 并拒绝未知能力.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn uppercase_preserves_whitespace_and_expands_unicode() {
    assert_eq!(
        execute(UPPERCASE.into(), " Beamlet ß\n世界\t".into())
            .await
            .unwrap(),
        " BEAMLET SS\n世界\t"
    );
    assert!(
        execute("text.uppercase/v2".into(), "x".into())
            .await
            .is_err()
    );
    assert!(validate(&BTreeSet::new()).is_err());
    assert!(validate(&["unknown/v1".into()].into()).is_err());
}
