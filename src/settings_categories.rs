//! Categories are presentation only: every driver field remains available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Frequent,
    Optics,
    Image,
    Tracking,
    Power,
    Advanced,
    All,
}
impl Category {
    pub const ALL: [Self; 7] = [
        Self::Frequent,
        Self::Optics,
        Self::Image,
        Self::Tracking,
        Self::Power,
        Self::Advanced,
        Self::All,
    ];
    pub fn includes(self, path: &str) -> bool {
        match self {
            Self::Frequent => priority(path) < 100,
            Self::All => true,
            _ => category(path) == self,
        }
    }
}
impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Frequent => "Frequently used",
            Self::Optics => "Optics & FOV",
            Self::Image => "Image & color",
            Self::Tracking => "Tracking & camera",
            Self::Power => "Power & presence",
            Self::Advanced => "Advanced",
            Self::All => "All settings",
        })
    }
}
pub fn priority(path: &str) -> usize {
    let key = path
        .trim_start_matches('/')
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .join("/");
    [
        "distortionProfile",
        "ipd",
        "ipdOffset",
        "hardwareIpd",
        "maxFovX",
        "maxFovY",
        "fovClamping",
        "brightness",
        "contrast",
        "saturation",
        "samplingFilter",
        "samplingFilterLumaSharpenStrength",
        "blackLevel",
        "enableEyeTracking",
        "enablePimaxPassthrough",
        "recenterPimaxPlayspace",
    ]
    .iter()
    .position(|k| *k == key)
    .unwrap_or(100)
}
pub fn category(path: &str) -> Category {
    let key = path.to_lowercase();
    if key.contains("stationarydimming") || key.contains("proximity") {
        Category::Power
    } else if key.contains("tracking")
        || key.contains("passthrough")
        || key.contains("recenter")
        || key.contains("bluetooth")
    {
        Category::Tracking
    } else if key.contains("distortion")
        || key.contains("fov")
        || key.contains("ipd")
        || key.contains("eyerotation")
        || key.contains("hiddenarea")
        || key.contains("parallelprojection")
        || key.contains("disableeye")
    {
        Category::Optics
    } else if key.starts_with("/customshader/")
        || key.contains("color")
        || key.contains("blacklevel")
        || key.contains("subpixel")
    {
        Category::Image
    } else {
        Category::Advanced
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn important_controls_sort_first_and_unknown_fields_remain_accessible() {
        assert!(priority("/dreamAir/distortionProfile") < priority("/dreamAir/ipd"));
        assert!(Category::Frequent.includes("/customShader/samplingFilter"));
        assert_eq!(
            category("/dreamAir/stationaryDimming/enable"),
            Category::Power
        );
        assert_eq!(
            category("/dreamAir/enablePimaxPassthrough"),
            Category::Tracking
        );
        assert_eq!(category("/dreamAir/newFutureField"), Category::Advanced);
        assert!(Category::All.includes("/newFutureField"));
    }
}
