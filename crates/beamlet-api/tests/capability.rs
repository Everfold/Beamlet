use beamlet_api::CapabilityId;

/// 验证能力标识长度按字节计算，并保留精确拼写。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn capability_validation_uses_bytes_and_preserves_the_exact_name() {
    let name = " Text.能力/v1 ";
    let id: CapabilityId = name.parse().unwrap();
    assert_eq!(id.as_str(), name);
    assert!("".parse::<CapabilityId>().is_err());
    assert!("é".repeat(128).parse::<CapabilityId>().is_ok());
    assert!("é".repeat(129).parse::<CapabilityId>().is_err());
}
