use std::ops::Range;

/// Dimension size.
pub type Size = u32;
/// Image layer.
pub type Layer = u32;
/// Image mipmap level.
pub type Level = u32;

/// Describes the size of an image, which may be up to three dimensional.
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Extent {
    /// Image width
    pub width: Size,
    /// Image height
    pub height: Size,
    /// Image depth.
    pub depth: Size,
}

impl Extent {
    /// Get the extent at a particular mipmap level.
    pub fn at_level(&self, level: Level) -> Self {
        Extent {
            width: 1.max(self.width >> level),
            height: 1.max(self.height >> level),
            depth: 1.max(self.depth >> level),
        }
    }
}

///
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Offset {
    ///
    pub x: i32,
    ///
    pub y: i32,
    ///
    pub z: i32,
}

impl Offset {
    /// Zero offset shortcut.
    pub const ZERO: Self = Offset { x: 0, y: 0, z: 0 };

    /// Convert the offset into 2-sided bounds given the extent.
    pub fn into_bounds(self, extent: &Extent) -> Range<Offset> {
        let end = Offset {
            x: self.x + extent.width as i32,
            y: self.y + extent.height as i32,
            z: self.z + extent.depth as i32,
        };
        self..end
    }
}
