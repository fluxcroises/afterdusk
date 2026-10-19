//! Jumeau GPU du pipeline (décision 0005) : le même calcul que
//! [`super::pipeline::render_pixel`], en WGSL par wgpu (Direct3D 12, Metal,
//! Vulkan). Les constantes du shader sont tirées du code processeur, qui reste
//! la référence ; sans processeur graphique utilisable, l'appelant revient au
//! rendu processeur.

use std::sync::{Arc, mpsc};

use wgpu::util::DeviceExt;

use super::pipeline::{REC2020_TO_SRGB, RenderParams, WorkingImage, process1};
use super::settings::DevelopSettings;

#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("aucun processeur graphique utilisable : {0}")]
    NoAdapter(#[from] wgpu::RequestAdapterError),
    #[error("processeur graphique indisponible : {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("image trop grande pour le processeur graphique ({0} octets)")]
    TooLarge(u64),
    #[error("erreur du processeur graphique : {0}")]
    Wgpu(String),
}

/// Taille d'un groupe de travail du shader, en pixels de côté.
const WORKGROUP: u32 = 16;

/// Le shader du processus 1, constantes comprises.
fn shader_source() -> String {
    let m = REC2020_TO_SRGB;
    let row = |r: [f32; 3]| format!("vec3<f32>({:?}, {:?}, {:?})", r[0], r[1], r[2]);
    format!(
        r#"
struct Params {{ gain: f32, width: u32, height: u32, pad: u32 }}

@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

const SHOULDER: f32 = {shoulder:?};
const M0 = {m0};
const M1 = {m1};
const M2 = {m2};

fn neutral_curve(x: f32) -> f32 {{
    if (x <= SHOULDER) {{
        return max(x, 0.0);
    }}
    return SHOULDER + (1.0 - SHOULDER) * (1.0 - exp(-(x - SHOULDER) / (1.0 - SHOULDER)));
}}

fn srgb_encode(v: f32) -> f32 {{
    let x = clamp(v, 0.0, 1.0);
    if (x <= 0.0031308) {{
        return 12.92 * x;
    }}
    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}}

@compute @workgroup_size({wg}, {wg})
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (id.x >= params.width || id.y >= params.height) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let lin = vec3<f32>(src[i * 3u], src[i * 3u + 1u], src[i * 3u + 2u]) * params.gain;
    let toned = vec3<f32>(neutral_curve(lin.x), neutral_curve(lin.y), neutral_curve(lin.z));
    let srgb = vec3<f32>(dot(M0, toned), dot(M1, toned), dot(M2, toned));
    dst[i] = pack4x8unorm(vec4<f32>(srgb_encode(srgb.x), srgb_encode(srgb.y), srgb_encode(srgb.z), 1.0));
}}
"#,
        shoulder = process1::SHOULDER,
        m0 = row(m[0]),
        m1 = row(m[1]),
        m2 = row(m[2]),
        wg = WORKGROUP,
    )
}

/// Processeur graphique prêt à rendre : appareil, file et pipeline compilé.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    adapter: String,
}

/// Image de travail chargée sur le processeur graphique, avec ses tampons.
pub struct GpuImage {
    pub width: u32,
    pub height: u32,
    params: wgpu::Buffer,
    dst: wgpu::Buffer,
    readback: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

fn wgpu_err(e: impl std::fmt::Display) -> GpuError {
    GpuError::Wgpu(e.to_string())
}

impl GpuRenderer {
    /// Choisit le processeur graphique le plus performant et compile le shader.
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(Self::init())
    }

    async fn init() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..Default::default()
            })
            .await?;
        let info = adapter.get_info();
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("afterdusk-develop"),
                required_limits: wgpu::Limits {
                    max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                    max_buffer_size: limits.max_buffer_size,
                    ..wgpu::Limits::default()
                },
                ..Default::default()
            })
            .await?;
        // Une erreur de validation ne doit jamais arrêter l'application : elle
        // est relevée par les portées d'erreur de chaque opération.
        device.on_uncaptured_error(Arc::new(|e| eprintln!("wgpu : {e}")));

        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("process1"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("process1"),
            entries: &[
                storage(0, true),
                storage(1, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("process1"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("process1"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = scope.pop().await {
            return Err(wgpu_err(e));
        }
        Ok(Self {
            device,
            queue,
            pipeline,
            layout,
            adapter: format!("{} ({:?})", info.name, info.backend),
        })
    }

    /// Nom du processeur graphique et interface utilisée.
    pub fn adapter_name(&self) -> &str {
        &self.adapter
    }

    /// Envoie l'image de travail au processeur graphique, une fois par photo.
    pub fn upload(&self, image: &WorkingImage) -> Result<GpuImage, GpuError> {
        let pixels = u64::from(image.width) * u64::from(image.height);
        let src_size = pixels * 12;
        let limits = self.device.limits();
        if src_size > limits.max_storage_buffer_binding_size || src_size > limits.max_buffer_size {
            return Err(GpuError::TooLarge(src_size));
        }
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let bytes: Vec<u8> = image.rgb.iter().flat_map(|v| v.to_le_bytes()).collect();
        let src = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("travail"),
                contents: &bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let dst = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rendu"),
            size: pixels * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lecture"),
            size: pixels * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("paramètres"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("process1"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: src.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: dst.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(wgpu_err(e));
        }
        Ok(GpuImage {
            width: image.width,
            height: image.height,
            params,
            dst,
            readback,
            bind_group,
        })
    }

    /// Rendu RGBA 8 bits, identique au rendu processeur à la tolérance près.
    pub fn render(
        &self,
        image: &GpuImage,
        settings: &DevelopSettings,
    ) -> Result<Vec<u8>, GpuError> {
        let p = RenderParams::from_settings(settings);
        let mut uniform = Vec::with_capacity(16);
        uniform.extend_from_slice(&p.gain.to_le_bytes());
        uniform.extend_from_slice(&image.width.to_le_bytes());
        uniform.extend_from_slice(&image.height.to_le_bytes());
        uniform.extend_from_slice(&0u32.to_le_bytes());

        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.queue.write_buffer(&image.params, 0, &uniform);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rendu"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("process1"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &image.bind_group, &[]);
            pass.dispatch_workgroups(
                image.width.div_ceil(WORKGROUP),
                image.height.div_ceil(WORKGROUP),
                1,
            );
        }
        encoder.copy_buffer_to_buffer(&image.dst, 0, &image.readback, 0, image.dst.size());
        self.queue.submit([encoder.finish()]);

        let (tx, rx) = mpsc::channel();
        image.readback.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(wgpu_err)?;
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(wgpu_err(e));
        }
        rx.recv().map_err(wgpu_err)?.map_err(wgpu_err)?;
        let out = image
            .readback
            .get_mapped_range(..)
            .map_err(wgpu_err)?
            .to_vec();
        image.readback.unmap();
        Ok(out)
    }
}
