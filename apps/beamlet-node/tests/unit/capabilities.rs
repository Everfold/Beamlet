use super::*;

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
