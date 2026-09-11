//! Regression tests for quaternion methods that delegate to `glam::Quat`.
//!
//! Each new implementation is checked against the previous hand-rolled formula
//! so the glam delegation stays behavior-compatible.

use kairos_math::{float3, quaternion};

struct Lcg(u64);

impl Lcg {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.0 >> 33) as u32 as f32) / (u32::MAX as f32)
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

fn legacy_from_euler(e: float3) -> [f32; 4] {
    let (sx, cx) = (e.x() * 0.5).sin_cos();
    let (sy, cy) = (e.y() * 0.5).sin_cos();
    let (sz, cz) = (e.z() * 0.5).sin_cos();

    let a = [sx * cy * cz, cx * sy * cz, cx * cy * sz, cx * cy * cz];
    let b = [cx * sy * sz, sx * cy * sz, sx * sy * cz, sx * sy * sz];
    let q = [a[0] - b[0], a[1] + b[1], a[2] - b[2], a[3] + b[3]];
    let inv = 1.0 / (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    [q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv]
}

fn legacy_to_euler(q: [f32; 4]) -> [f32; 3] {
    let inv = 1.0 / (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let (x, y, z, w) = (q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv);
    let sin_x = 2.0 * (w * x + y * z);
    let cos_x = 1.0 - 2.0 * (x * x + y * y);
    let sin_y = 2.0 * (w * y - z * x);
    let sin_z = 2.0 * (w * z + x * y);
    let cos_z = 1.0 - 2.0 * (y * y + z * z);
    [
        sin_x.atan2(cos_x),
        sin_y.clamp(-1.0, 1.0).asin(),
        sin_z.atan2(cos_z),
    ]
}

fn legacy_from_look(forward: float3, up_world: float3) -> [f32; 4] {
    let f = glam::Vec3::new(forward.x(), forward.y(), forward.z()).normalize();
    let upw = glam::Vec3::new(up_world.x(), up_world.y(), up_world.z());
    let mut r = f.cross(upw);
    let r_len = r.length();

    let (right, up) = if r_len < 1e-7 {
        let alt_up = if upw.y.abs() > 0.999 {
            glam::Vec3::X
        } else {
            glam::Vec3::Y
        };
        let right = f.cross(alt_up).normalize();
        let up = right.cross(f);
        (right, up)
    } else {
        r *= 1.0 / r_len;
        let up = r.cross(f);
        (r, up)
    };

    let m00 = right.x;
    let m01 = up.x;
    let m02 = -f.x;
    let m10 = right.y;
    let m11 = up.y;
    let m12 = -f.y;
    let m20 = right.z;
    let m21 = up.z;
    let m22 = -f.z;

    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, s * 0.25]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [s * 0.25, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, s * 0.25, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, s * 0.25, (m10 - m01) / s]
    }
}

fn quat_abs_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs()
}

#[test]
fn from_euler_matches_legacy_formula() {
    let mut rng = Lcg(0x9e3779b97f4a7c15);
    for _ in 0..2000 {
        let e = float3::new(
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
        );
        let got = quaternion::from_euler(e).0.to_array();
        let want = legacy_from_euler(e);
        assert!(
            quat_abs_dot(got, want) > 1.0 - 1e-5,
            "euler={e:?} got={got:?} want={want:?}"
        );
    }
}

#[test]
fn to_euler_matches_legacy_formula() {
    let mut rng = Lcg(0xdeadbeefcafebabe);
    for _ in 0..2000 {
        // Keep away from the pitch gimbal-lock singularity.
        let q = quaternion::from_euler(float3::new(
            rng.range(-1.2, 1.2),
            rng.range(-1.2, 1.2),
            rng.range(-3.0, 3.0),
        ));
        let got = q.to_euler();
        let want = legacy_to_euler(q.0.to_array());
        let err = (got.x() - want[0])
            .abs()
            .max((got.y() - want[1]).abs())
            .max((got.z() - want[2]).abs());
        assert!(err < 1e-4, "got={got:?} want={want:?}");
    }
}

#[test]
fn from_euler_to_euler_round_trips() {
    let mut rng = Lcg(0x0123456789abcdef);
    for _ in 0..2000 {
        let e = float3::new(
            rng.range(-1.2, 1.2),
            rng.range(-1.2, 1.2),
            rng.range(-3.0, 3.0),
        );
        let got = quaternion::from_euler(e).to_euler();
        let err = (got.x() - e.x())
            .abs()
            .max((got.y() - e.y()).abs())
            .max((got.z() - e.z()).abs());
        assert!(err < 1e-4, "euler={e:?} got={got:?}");
    }
}

#[test]
fn from_look_matches_legacy_formula() {
    let mut rng = Lcg(0xfedcba9876543210);
    for _ in 0..2000 {
        let forward = float3::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        );
        let up = float3::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        );
        let got = quaternion::from_look(forward, up).0.to_array();
        let want = legacy_from_look(forward, up);
        assert!(
            quat_abs_dot(got, want) > 1.0 - 1e-4,
            "forward={forward:?} up={up:?} got={got:?} want={want:?}"
        );
    }
}

#[test]
fn from_look_handles_parallel_forward_and_up() {
    let forward = float3::new(0.0, 1.0, 0.0);
    let up = float3::new(0.0, 1.0, 0.0);
    let got = quaternion::from_look(forward, up).0.to_array();
    let want = legacy_from_look(forward, up);
    assert!(quat_abs_dot(got, want) > 1.0 - 1e-4);
}

#[test]
fn mul_matches_component_formula() {
    let mut rng = Lcg(0x1111222233334444);
    for _ in 0..2000 {
        let a = quaternion::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        );
        let b = quaternion::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        );
        let (ax, ay, az, aw) = (a.0.x, a.0.y, a.0.z, a.0.w);
        let (bx, by, bz, bw) = (b.0.x, b.0.y, b.0.z, b.0.w);
        let want = [
            aw * bx + bw * ax + ay * bz - az * by,
            aw * by + bw * ay + az * bx - ax * bz,
            aw * bz + bw * az + ax * by - ay * bx,
            aw * bw - ax * bx - ay * by - az * bz,
        ];
        let got = (a * b).0.to_array();
        let err = (0..4)
            .map(|i| (got[i] - want[i]).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-5, "got={got:?} want={want:?}");
    }
}
