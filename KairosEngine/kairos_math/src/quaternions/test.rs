use crate::quaternion;

#[test]
fn quaternion_serializes_as_four_element_array() {
    let q = quaternion::new(1.0, 2.0, 3.0, 4.0);
    let json = serde_json::to_string(&q).unwrap();
    assert_eq!(json, "[1.0,2.0,3.0,4.0]");
}

#[test]
fn quaternion_serde_roundtrip() {
    let q = quaternion::new(0.1, 0.2, 0.3, 0.9).normalized();
    let json = serde_json::to_string(&q).unwrap();
    let back: quaternion = serde_json::from_str(&json).unwrap();
    assert_eq!(q, back);
}
