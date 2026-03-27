//! Pixel data for canvas image operations.
//!
//! Chrome equivalent: `ImageData`.

/// Raw RGBA pixel data.
///
/// Chrome equivalent: `ImageData` — the backing store for
/// `getImageData()`/`putImageData()`/`createImageData()`.
/// Pixels are stored in row-major order, 4 bytes per pixel (R, G, B, A).
#[derive(Clone, Debug)]
pub struct ImageData {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl ImageData {
    /// Create a new `ImageData` with the given dimensions, filled with transparent black.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            data: vec![0; len],
        }
    }

    /// Create from existing pixel data. Returns `None` if `data.len()` doesn't
    /// match `width * height * 4`.
    #[must_use]
    pub fn from_data(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        let expected = (width as usize) * (height as usize) * 4;
        if data.len() != expected {
            return None;
        }
        Some(Self {
            width,
            height,
            data,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}
