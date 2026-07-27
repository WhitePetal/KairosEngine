use crate::math::{float4, float4x4};

#[test]
fn identity_matrix_multiply() {
    let m = float4x4::new(
        float4::new(1.0, 2.0, 3.0, 4.0),
        float4::new(5.0, 6.0, 7.0, 8.0),
        float4::new(9.0, 10.0, 11.0, 12.0),
        float4::new(13.0, 14.0, 15.0, 16.0),
    );
    let identity = float4x4::IDENTITY;

    assert_eq!(identity * m, m);
    assert_eq!(m * identity, m);
}

#[test]
fn column_major_matrix_multiply() {
    let lhs = float4x4::new(
        float4::new(1.0, 2.0, 3.0, 4.0),
        float4::new(5.0, 6.0, 7.0, 8.0),
        float4::new(9.0, 10.0, 11.0, 12.0),
        float4::new(13.0, 14.0, 15.0, 16.0),
    );
    let rhs = float4x4::new(
        float4::new(17.0, 18.0, 19.0, 20.0),
        float4::new(21.0, 22.0, 23.0, 24.0),
        float4::new(25.0, 26.0, 27.0, 28.0),
        float4::new(29.0, 30.0, 31.0, 32.0),
    );

    assert_eq!(
        lhs * rhs,
        float4x4::new(
            float4::new(538.0, 612.0, 686.0, 760.0),
            float4::new(650.0, 740.0, 830.0, 920.0),
            float4::new(762.0, 868.0, 974.0, 1080.0),
            float4::new(874.0, 996.0, 1118.0, 1240.0),
        )
    );
}
