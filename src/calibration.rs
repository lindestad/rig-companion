use anyhow::{Result, ensure};
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pose {
    pub position: [f64; 3],
    pub yaw: f64,
}

impl Pose {
    pub fn validate(self) -> Result<Self> {
        ensure!(
            self.position.iter().all(|v| v.is_finite()) && self.yaw.is_finite(),
            "Invalid headset pose"
        );
        Ok(self)
    }
}

pub fn validate_height(height: f64) -> Result<f64> {
    ensure!(
        height.is_finite() && (0.2..=2.5).contains(&height),
        "Seated height must be between 20 and 250 cm"
    );
    Ok(height)
}

/// A world-space correction. OpenVR's setup API needs the inverse mapping.
pub fn height_correction(current: Pose, target: f64) -> Result<DMat4> {
    current.validate()?;
    validate_height(target)?;
    Ok(DMat4::from_translation(
        DVec3::Y * (target - current.position[1]),
    ))
}

pub fn reference_correction(current: Pose, target: Pose) -> Result<DMat4> {
    current.validate()?;
    target.validate()?;
    validate_height(target.position[1])?;
    let rotation = DMat4::from_rotation_y(target.yaw - current.yaw);
    let translation =
        DVec3::from(target.position) - rotation.transform_point3(current.position.into());
    Ok(DMat4::from_translation(translation) * rotation)
}

pub fn corrected_origin(standing_to_raw: DMat4, correction: DMat4) -> DMat4 {
    standing_to_raw * correction.inverse()
}

pub fn from_rows(rows: [[f32; 4]; 3]) -> DMat4 {
    DMat4::from_cols_array(&[
        rows[0][0] as f64,
        rows[1][0] as f64,
        rows[2][0] as f64,
        0.0,
        rows[0][1] as f64,
        rows[1][1] as f64,
        rows[2][1] as f64,
        0.0,
        rows[0][2] as f64,
        rows[1][2] as f64,
        rows[2][2] as f64,
        0.0,
        rows[0][3] as f64,
        rows[1][3] as f64,
        rows[2][3] as f64,
        1.0,
    ])
}

pub fn to_rows(matrix: DMat4) -> [[f32; 4]; 3] {
    let c = matrix.to_cols_array_2d();
    std::array::from_fn(|r| std::array::from_fn(|col| c[col][r] as f32))
}

pub fn pose_from_matrix(matrix: DMat4) -> Result<Pose> {
    // OpenVR forward is -Z. Ignore pitch/roll when deriving a level yaw.
    let forward = matrix.transform_vector3(DVec3::NEG_Z);
    ensure!(
        forward.x.hypot(forward.z) > 0.1,
        "Look forward before capturing a reference"
    );
    Pose {
        position: matrix.w_axis.truncate().to_array(),
        yaw: (-forward.x).atan2(-forward.z),
    }
    .validate()
}

pub fn matrix_close(a: DMat4, b: DMat4) -> bool {
    a.to_cols_array()
        .iter()
        .zip(b.to_cols_array())
        .all(|(a, b)| (a - b).abs() < 0.0001)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_floor_restore_has_correct_sign_and_is_idempotent() {
        let current = Pose {
            position: [0.2, -0.3, 0.4],
            yaw: 0.0,
        };
        let correction = height_correction(current, 1.15).unwrap();
        let origin = corrected_origin(DMat4::IDENTITY, correction);
        let corrected = origin.inverse().transform_point3(current.position.into());
        assert!((corrected.y - 1.15).abs() < 1e-9);
        assert_eq!(corrected.x, 0.2);
        let second = height_correction(
            Pose {
                position: corrected.to_array(),
                ..current
            },
            1.15,
        )
        .unwrap();
        assert!(matrix_close(second, DMat4::IDENTITY));
    }

    #[test]
    fn full_restore_composes_with_rotated_existing_origin() {
        let old = DMat4::from_rotation_y(0.8) * DMat4::from_translation(DVec3::new(2.0, -1.0, 3.0));
        let raw =
            DMat4::from_translation(DVec3::new(1.0, 0.4, -2.0)) * DMat4::from_rotation_y(-0.6);
        let current = pose_from_matrix(old.inverse() * raw).unwrap();
        let target = Pose {
            position: [0.0, 1.1, 0.0],
            yaw: 0.2,
        };
        let new = corrected_origin(old, reference_correction(current, target).unwrap());
        let result = pose_from_matrix(new.inverse() * raw).unwrap();
        assert!((DVec3::from(result.position) - DVec3::from(target.position)).length() < 1e-9);
        assert!((result.yaw - target.yaw).abs() < 1e-9);
    }

    #[test]
    fn matrix_layout_round_trip() {
        let m = DMat4::from_translation(DVec3::new(3.0, 1.2, -2.0)) * DMat4::from_rotation_y(0.9);
        assert!(matrix_close(m, from_rows(to_rows(m))));
    }

    #[test]
    fn rejects_nonfinite_and_implausible_references() {
        for h in [f64::NAN, f64::INFINITY, -0.5, 0.0, 10.0] {
            assert!(validate_height(h).is_err());
        }
        assert!(
            reference_correction(
                Pose {
                    position: [f64::NAN, 1.0, 0.0],
                    yaw: 0.0
                },
                Pose {
                    position: [0.0, 1.1, 0.0],
                    yaw: 0.0
                }
            )
            .is_err()
        );
    }
}
