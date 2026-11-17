use beamlet_api::CapabilityId;

#[test]
fn capability_validation_uses_bytes_and_preserves_the_exact_name() {
    let name = " Text.能力/v1 ";
    let id: CapabilityId = name.parse().unwrap();
    assert_eq!(id.as_str(), name);
    assert!("".parse::<CapabilityId>().is_err());
    assert!("é".repeat(128).parse::<CapabilityId>().is_ok());
    assert!("é".repeat(129).parse::<CapabilityId>().is_err());
}
