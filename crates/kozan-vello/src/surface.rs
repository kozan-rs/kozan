//! `VelloSurface` — per-window rendering target backed by vello + wgpu.
//!
//! Uses `vello::wgpu` exclusively to avoid version conflicts.

use std::sync::Arc;

use vello::wgpu;
use vello::{AaConfig, RenderParams as VelloRenderParams};

use kozan_platform::{RendererError, RenderParams, RenderSurface};

use crate::renderer::GpuContext;
use crate::scene::SceneBuilder;

/// Per-window rendering surface. Owns the wgpu swap chain + vello `Renderer`.
pub struct VelloSurface {
    gpu: Arc<GpuContext>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    vello: vello::Renderer,
    width: u32,
    height: u32,
}

impl VelloSurface {
    /// Create a new surface. Called by `VelloRenderer::create_surface`.
    pub(crate) fn new(
        gpu: Arc<GpuContext>,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, RendererError> {
        let surface_caps = surface.get_capabilities(&gpu.adapter);

        // Vello's render_to_surface only accepts Bgra8Unorm or Rgba8Unorm (non-sRGB).
        // Prefer Bgra8Unorm (most common on Windows/macOS), then Rgba8Unorm.
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| {
                matches!(
                    **f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &config);

        let vello = vello::Renderer::new(
            &gpu.device,
            vello::RendererOptions {
                surface_format: Some(surface_format),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
            },
        )
        .map_err(|e| RendererError::RenderFailed(e.to_string()))?;

        Ok(Self {
            gpu,
            surface,
            surface_format,
            vello,
            width: width.max(1),
            height: height.max(1),
        })
    }

    fn reconfigure(&self) {
        self.surface.configure(
            &self.gpu.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format,
                width: self.width,
                height: self.height,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vello_surface_is_send() {
        fn _assert_send<T: Send>() {}
        _assert_send::<VelloSurface>();
    }
}

impl RenderSurface for VelloSurface {
    fn render(&mut self, params: &RenderParams) -> Result<(), RendererError> {
        if params.width != self.width || params.height != self.height {
            self.resize(params.width, params.height);
        }

        let scene = SceneBuilder::build(
            &params.frame.display_list,
            params.scale_factor,
            &params.frame.scroll_offsets,
        );

        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                return Err(RendererError::SurfaceLost);
            }
            Err(e) => {
                return Err(RendererError::SurfaceTextureAcquire(e.to_string()));
            }
        };

        self.vello
            .render_to_surface(
                &self.gpu.device,
                &self.gpu.queue,
                &scene,
                &surface_texture,
                &VelloRenderParams {
                    base_color: vello::peniko::Color::BLACK,
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| RendererError::RenderFailed(e.to_string()))?;

        surface_texture.present();
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.reconfigure();
    }
}
