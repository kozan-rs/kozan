//! Renderer errors — unified error type for all renderer backends.

use std::fmt;

/// Error returned by renderer operations.
///
/// Backend-independent: both `kozan-vello` and any future backends
/// map their internal errors into this type.
#[derive(Debug)]
pub enum RendererError {
    /// Failed to acquire the GPU adapter.
    AdapterNotFound,

    /// Failed to create the GPU device.
    DeviceCreation(String),

    /// Failed to create the window surface.
    SurfaceCreation(String),

    /// The surface is not compatible with the adapter.
    SurfaceIncompatible,

    /// Failed to acquire the next surface texture.
    SurfaceTextureAcquire(String),

    /// The renderer backend produced an error during rendering.
    RenderFailed(String),

    /// The surface was lost (e.g., window minimized or resized race).
    /// Caller should recreate the surface.
    SurfaceLost,
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterNotFound => write!(f, "no suitable GPU adapter found"),
            Self::DeviceCreation(e) => write!(f, "GPU device creation failed: {e}"),
            Self::SurfaceCreation(e) => write!(f, "surface creation failed: {e}"),
            Self::SurfaceIncompatible => {
                write!(f, "surface is not compatible with the GPU adapter")
            }
            Self::SurfaceTextureAcquire(e) => write!(f, "failed to acquire surface texture: {e}"),
            Self::RenderFailed(e) => write!(f, "render failed: {e}"),
            Self::SurfaceLost => write!(f, "surface lost — recreate required"),
        }
    }
}

impl std::error::Error for RendererError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_adapter_not_found() {
        let err = RendererError::AdapterNotFound;
        assert_eq!(err.to_string(), "no suitable GPU adapter found");
    }

    #[test]
    fn display_device_creation_includes_inner_message() {
        let err = RendererError::DeviceCreation("out of memory".into());
        assert_eq!(err.to_string(), "GPU device creation failed: out of memory");
    }

    #[test]
    fn display_surface_creation_includes_inner_message() {
        let err = RendererError::SurfaceCreation("unsupported format".into());
        assert_eq!(
            err.to_string(),
            "surface creation failed: unsupported format"
        );
    }

    #[test]
    fn display_surface_incompatible() {
        let err = RendererError::SurfaceIncompatible;
        assert_eq!(
            err.to_string(),
            "surface is not compatible with the GPU adapter"
        );
    }

    #[test]
    fn display_surface_texture_acquire_includes_inner_message() {
        let err = RendererError::SurfaceTextureAcquire("timeout".into());
        assert_eq!(
            err.to_string(),
            "failed to acquire surface texture: timeout"
        );
    }

    #[test]
    fn display_render_failed_includes_inner_message() {
        let err = RendererError::RenderFailed("shader compilation".into());
        assert_eq!(err.to_string(), "render failed: shader compilation");
    }

    #[test]
    fn display_surface_lost() {
        let err = RendererError::SurfaceLost;
        assert_eq!(err.to_string(), "surface lost — recreate required");
    }

    #[test]
    fn debug_includes_variant_name() {
        let err = RendererError::AdapterNotFound;
        let debug = format!("{err:?}");
        assert!(debug.contains("AdapterNotFound"));
    }

    #[test]
    fn implements_std_error() {
        let err = RendererError::RenderFailed("test".into());
        let std_err: &dyn std::error::Error = &err;
        assert!(std_err.source().is_none());
    }
}
