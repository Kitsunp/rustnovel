use pollster::FutureExt;
use std::sync::Arc;
use wgpu::{
    Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor, CurrentSurfaceTexture, Device,
    DeviceDescriptor, Extent3d, Features, FilterMode, FragmentState, Instance, Limits, LoadOp,
    MultisampleState, Operations, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PowerPreference, PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Surface,
    SurfaceConfiguration, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
};
use winit::window::Window;

use super::backend::{RenderBackend, RenderFrame};
use super::software::{BuiltinSoftwareDrawer, SoftwareDrawStrategy};

const BLIT_SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );

    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(frame_texture, frame_sampler, in.uv);
}
"#;

pub struct WgpuBackend<'a> {
    surface: Surface<'a>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    drawer: BuiltinSoftwareDrawer,
    cpu_frame: Vec<u8>,
    frame_texture: Texture,
    frame_view: TextureView,
    frame_bind_group: BindGroup,
    frame_bind_group_layout: BindGroupLayout,
    frame_sampler: Sampler,
    pipeline: RenderPipeline,
}

impl<'a> WgpuBackend<'a> {
    pub fn new(window: Arc<Window>, width: u32, height: u32) -> Result<Self, String> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = Backends::all();
        let instance = Instance::new(instance_descriptor);

        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .block_on()
            .map_err(|err| format!("failed to find an appropriate adapter: {err}"))?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("Wgpu Device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                ..Default::default()
            })
            .block_on()
            .map_err(|e| e.to_string())?;

        let caps = surface.get_capabilities(&adapter);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let frame_bind_group_layout = create_frame_bind_group_layout(&device);
        let frame_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Runtime Frame Sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let (frame_texture, frame_view, frame_bind_group) = create_frame_texture(
            &device,
            &frame_bind_group_layout,
            &frame_sampler,
            width,
            height,
        );
        let pipeline = create_blit_pipeline(&device, config.format, &frame_bind_group_layout);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            drawer: BuiltinSoftwareDrawer::new(),
            cpu_frame: Vec::new(),
            frame_texture,
            frame_view,
            frame_bind_group,
            frame_bind_group_layout,
            frame_sampler,
            pipeline,
        })
    }

    fn recreate_frame_texture(&mut self) {
        let (texture, view, bind_group) = create_frame_texture(
            &self.device,
            &self.frame_bind_group_layout,
            &self.frame_sampler,
            self.config.width,
            self.config.height,
        );
        self.frame_texture = texture;
        self.frame_view = view;
        self.frame_bind_group = bind_group;
    }
}

impl<'a> RenderBackend for WgpuBackend<'a> {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.recreate_frame_texture();
        }
        Ok(())
    }

    fn render(&mut self, render_frame: RenderFrame<'_>) -> Result<(), String> {
        let width = self.config.width.max(1);
        let height = self.config.height.max(1);
        let expected_len = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if self.cpu_frame.len() != expected_len {
            self.cpu_frame.resize(expected_len, 0);
        }
        self.drawer
            .draw(&mut self.cpu_frame, (width, height), render_frame);
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &self.cpu_frame,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width.saturating_mul(4)),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let output = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture)
            | CurrentSurfaceTexture::Suboptimal(texture) => texture,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(()),
            CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(texture)
                    | CurrentSurfaceTexture::Suboptimal(texture) => texture,
                    CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                        return Ok(());
                    }
                    other => {
                        return Err(format!(
                            "failed to acquire surface texture after reconfigure: {other:?}"
                        ));
                    }
                }
            }
            other => return Err(format!("failed to acquire surface texture: {other:?}")),
        };
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Runtime Blit Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.frame_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn create_frame_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Runtime Frame Bind Group Layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_frame_texture(
    device: &Device,
    layout: &BindGroupLayout,
    sampler: &Sampler,
    width: u32,
    height: u32,
) -> (Texture, TextureView, BindGroup) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Runtime CPU Frame Texture"),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Runtime Frame Bind Group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, view, bind_group)
}

fn create_blit_pipeline(
    device: &Device,
    format: TextureFormat,
    bind_group_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Runtime Frame Blit Shader"),
        source: ShaderSource::Wgsl(BLIT_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Runtime Frame Blit Pipeline Layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Runtime Frame Blit Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
