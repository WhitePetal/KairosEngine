use super::*;

#[test]
fn test_pascal_to_snake_case() {
    assert_eq!(pascal_to_snake_case("PascalCase"), "pascal_case");
    assert_eq!(pascal_to_snake_case("lowercase"), "lowercase");
    assert_eq!(pascal_to_snake_case("HTTPServer"), "http_server");
}
