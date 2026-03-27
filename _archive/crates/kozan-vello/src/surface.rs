//! `VelloSurface` — per-window rendering target backed by vello + wgpu.
//!
//! Uses `vello::wgpu` exclusively to avoid version conflicts.

use std::sync::Arc;

use vello::wgpu;
use vello::{AaConfig, RenderParams as VelloRenderParams};

use kozan_platform::{RenderParams, RenderSurface, RendererError};

use crate::renderer::GpuContext;
use crate::scene::SceneBuilder;

/// Per-window rendering surface. Owns the wgpu swap chain + vello `Renderer`.
///
/// Vello 0.5+ removed `render_to_surface`. The pipeline is now:
/// 1. Render to an intermediate `Rgba8Unorm` texture via `render_to_texture`
/// 2. Blit that texture to the swap chain surface via `TextureBlitter`
pub struct VelloSurface {
    gpu: Arc<GpuContext>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    vello: vello::Renderer,
    blitter: wgpu::util::TextureBlitter,
    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// Called just before `surface_texture.present()`.
    /// On X11: increments the `_NET_WM_SYNC_REQUEST` counter so the WM
    /// doesn't show the new window size until this frame is ready.
    pre_present_hook: Option<Box<dyn Fn() + Send>>,
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

        let w = width.max(1);
        let h = height.max(1);

        surface.configure(
            &gpu.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: w,
                height: h,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            },
        );

        let vello = vello::Renderer::new(
            &gpu.device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::RenderFailed(e.to_string()))?;

        let blitter = wgpu::util::TextureBlitter::new(&gpu.device, surface_format);
        let (target_texture, target_view) = Self::create_target_texture(&gpu.device, w, h);

        Ok(Self {
            gpu,
            surface,
            surface_format,
            vello,
            blitter,
            target_texture,
            target_view,
            width: w,
            height: h,
            pre_present_hook: None,
        })
    }

    fn create_target_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kozan-vello-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn reconfigure(&mut self) {
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
                // Single frame in flight — minimises the window where the
                // Wayland compositor shows a stale buffer during resize.
                desired_maximum_frame_latency: 1,
            },
        );
        let (texture, view) =
            Self::create_target_texture(&self.gpu.device, self.width, self.height);
        self.target_texture = texture;
        self.target_view = view;
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
            params.content_scale,
            params.device_scale,
            &params.frame.scroll_offsets,
            &params.frame.quads,
        );

        self.vello
            .render_to_texture(
                &self.gpu.device,
                &self.gpu.queue,
                &scene,
                &self.target_view,
                &VelloRenderParams {
                    base_color: vello::peniko::color::palette::css::BLACK,
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| RendererError::RenderFailed(e.to_string()))?;

        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Reconfigure and retry once — handles minimize, maximize,
                // and display mode changes without killing the window.
                self.reconfigure();
                match self.surface.get_current_texture() {
                    Ok(t) => t,
                    // Still failing (e.g., window minimized to 0x0) — skip
                    // this frame. The next resize/redraw will recover.
                    Err(_) => return Ok(()),
                }
            }
            Err(e) => {
                return Err(RendererError::SurfaceTextureAcquire(e.to_string()));
            }
        };

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kozan-blit"),
            });
        self.blitter.copy(
            &self.gpu.device,
            &mut encoder,
            &self.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        self.gpu.queue.submit([encoder.finish()]);
        if let Some(hook) = &self.pre_present_hook {
            hook();
        }
        surface_texture.present();

        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.reconfigure();
    }

    fn set_pre_present_hook(&mut self, hook: Box<dyn Fn() + Send>) {
        self.pre_present_hook = Some(hook);
    }
}
