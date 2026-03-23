//! `VelloRenderer` — the vello + wgpu implementation of `Renderer`.
//!
//! Uses `vello::wgpu` re-export to guarantee version consistency with vello.
//! No direct wgpu dependency in Cargo.toml — single source of truth.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vello::wgpu;

use kozan_platform::{Renderer, RendererError};

use crate::surface::VelloSurface;

/// Shared GPU resources — one device + queue per process.
///
/// Wrapped in `Arc` so `VelloSurface`s can hold a reference without
/// lifetime coupling to `VelloRenderer`.
pub(crate) struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Vello + wgpu renderer — the process-level GPU owner.
pub struct VelloRenderer {
    gpu: Arc<GpuContext>,
}

impl VelloRenderer {
    /// Create the vello renderer.
    ///
    /// Blocks the calling thread while requesting the GPU adapter and device.
    /// Call once at app startup, before the winit event loop.
    pub fn new() -> Result<Self, RendererError> {
        pollster::block_on(Self::init())
    }

    async fn init() -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| RendererError::AdapterNotFound)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kozan-vello"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e: wgpu::RequestDeviceError| RendererError::DeviceCreation(e.to_string()))?;

        Ok(Self {
            gpu: Arc::new(GpuContext { instance, adapter, device, queue }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vello_renderer_is_send_and_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<VelloRenderer>();
    }

    #[test]
    fn gpu_context_fields_are_accessible() {
        fn _takes_ctx(ctx: &GpuContext) {
            let _: &wgpu::Instance = &ctx.instance;
            let _: &wgpu::Adapter = &ctx.adapter;
            let _: &wgpu::Device = &ctx.device;
            let _: &wgpu::Queue = &ctx.queue;
        }
    }
}

impl Renderer for VelloRenderer {
    type Surface = VelloSurface;

    fn create_surface<W>(
        &self,
        window: &W,
        width: u32,
        height: u32,
    ) -> Result<Self::Surface, RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle,
    {
        // SAFETY: The window outlives the surface.
        // kozan-winit's WindowState owns both and drops the surface before the window.
        let surface = unsafe {
            self.gpu
                .instance
                .create_surface_unsafe(
                    wgpu::SurfaceTargetUnsafe::from_window(window)
                        .map_err(|e| RendererError::SurfaceCreation(e.to_string()))?,
                )
                .map_err(|e| RendererError::SurfaceCreation(e.to_string()))?
        };

        VelloSurface::new(Arc::clone(&self.gpu), surface, width, height)
    }
}
