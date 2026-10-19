//! Jumeau GPU du pipeline (décision 0005) : le même calcul que
//! [`super::pipeline::render`], en WGSL par wgpu (Direct3D 12, Metal,
//! Vulkan). Les constantes du shader sont tirées du code processeur, qui reste
//! la référence ; sans processeur graphique utilisable, l'appelant revient au
//! rendu processeur.

use std::sync::{Arc, mpsc};

use wgpu::util::DeviceExt;

use super::pipeline::{
    CLIP_HIGHLIGHT_RGB, CLIP_SHADOW_RGB, FLAG_CLIP_HIGHLIGHTS, FLAG_CLIP_SHADOWS, FLAG_TONE,
    HISTOGRAM_BINS, LUMA_REC2020, REC2020_TO_SRGB, RenderParams, Rendered, WorkingImage, process1,
};

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

/// Taille d'un groupe de travail du shader, en pixels de côté (16 × 16 = 256
/// invocations, chacune chargée de quatre cases de l'histogramme local).
const WORKGROUP: u32 = 16;

/// Taille du bloc de paramètres (voir `Params` dans le shader).
const UNIFORM_SIZE: u64 = 96;

const HISTOGRAM_BYTES: u64 = HISTOGRAM_BINS as u64 * 4;

fn vec3(v: [f32; 3]) -> String {
    format!("vec3<f32>({:?}, {:?}, {:?})", v[0], v[1], v[2])
}

fn rgb_u32(c: [u8; 3]) -> String {
    format!("vec3<u32>({}u, {}u, {}u)", c[0], c[1], c[2])
}

/// Le shader du processus 1, constantes comprises.
fn shader_source() -> String {
    let m = REC2020_TO_SRGB;
    format!(
        r#"
struct Params {{
    m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>,
    tone_a: vec4<f32>,
    tone_b: vec4<f32>,
    width: u32, height: u32, flags: u32, pad: u32,
}}

@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> hist: array<atomic<u32>, {bins}>;

var<workgroup> local_hist: array<atomic<u32>, {bins}>;

const SHOULDER: f32 = {shoulder:?};
const TONE_GAMMA: f32 = {gamma:?};
const TONE_MID: f32 = {mid:?};
const LUMA = {luma};
const S0 = {s0};
const S1 = {s1};
const S2 = {s2};
const FLAG_TONE: u32 = {flag_tone}u;
const FLAG_CLIP_HIGHLIGHTS: u32 = {flag_hi}u;
const FLAG_CLIP_SHADOWS: u32 = {flag_lo}u;
const CLIP_HIGHLIGHT = {clip_hi};
const CLIP_SHADOW = {clip_lo};

fn neutral_curve(x: f32) -> f32 {{
    if (x <= SHOULDER) {{
        return max(x, 0.0);
    }}
    return SHOULDER + (1.0 - SHOULDER) * (1.0 - exp(-(x - SHOULDER) / (1.0 - SHOULDER)));
}}

fn tone_curve(p: f32) -> f32 {{
    let a = params.tone_a;
    let b = params.tone_b;
    let w = 1.0 - p;
    let out = p
        + a.x * p * w
        + a.y * 4.0 * (p - TONE_MID) * p * w
        + a.z * p * p * p * w
        + a.w * p * w * w * w
        + b.x * p * p * p * p
        + b.y * w * w * w * w;
    return max(out, 0.0);
}}

fn srgb_encode(v: f32) -> f32 {{
    let x = clamp(v, 0.0, 1.0);
    if (x <= 0.0031308) {{
        return 12.92 * x;
    }}
    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}}

fn render_pixel(rgb: vec3<f32>) -> vec3<f32> {{
    let lin = vec3<f32>(dot(params.m0.xyz, rgb), dot(params.m1.xyz, rgb), dot(params.m2.xyz, rgb));
    var v = vec3<f32>(neutral_curve(lin.x), neutral_curve(lin.y), neutral_curve(lin.z));
    if ((params.flags & FLAG_TONE) != 0u) {{
        let y = dot(LUMA, v);
        if (y > 1e-6) {{
            let t = tone_curve(pow(y, 1.0 / TONE_GAMMA));
            var ratio = 0.0;
            if (t > 0.0) {{
                ratio = pow(t, TONE_GAMMA) / y;
            }}
            v = v * ratio;
        }}
    }}
    let saturation = params.tone_b.z;
    let vibrance = params.tone_b.w;
    if (saturation != 0.0 || vibrance != 0.0) {{
        let y = dot(LUMA, v);
        let mx = max(v.x, max(v.y, v.z));
        let mn = min(v.x, min(v.y, v.z));
        var chroma = 0.0;
        if (mx > 1e-6) {{
            chroma = clamp((mx - mn) / mx, 0.0, 1.0);
        }}
        let dull = 1.0 - chroma;
        let factor = (1.0 + saturation) * (1.0 + vibrance * dull * dull);
        v = vec3<f32>(y) + (v - vec3<f32>(y)) * factor;
    }}
    let srgb = vec3<f32>(dot(S0, v), dot(S1, v), dot(S2, v));
    return vec3<f32>(srgb_encode(srgb.x), srgb_encode(srgb.y), srgb_encode(srgb.z));
}}

fn quantize(v: f32) -> u32 {{
    return u32(clamp(v, 0.0, 1.0) * 255.0 + 0.5);
}}

@compute @workgroup_size({wg}, {wg})
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {{
    for (var k = 0u; k < 4u; k++) {{
        atomicStore(&local_hist[li * 4u + k], 0u);
    }}
    workgroupBarrier();
    if (id.x < params.width && id.y < params.height) {{
        let i = id.y * params.width + id.x;
        let v = render_pixel(vec3<f32>(src[i * 3u], src[i * 3u + 1u], src[i * 3u + 2u]));
        let q = vec3<u32>(quantize(v.x), quantize(v.y), quantize(v.z));
        atomicAdd(&local_hist[q.x], 1u);
        atomicAdd(&local_hist[256u + q.y], 1u);
        atomicAdd(&local_hist[512u + q.z], 1u);
        let l = 0.2126 * f32(q.x) + 0.7152 * f32(q.y) + 0.0722 * f32(q.z);
        atomicAdd(&local_hist[768u + min(u32(l + 0.5), 255u)], 1u);
        var shown = q;
        if ((params.flags & FLAG_CLIP_HIGHLIGHTS) != 0u && any(q == vec3<u32>(255u))) {{
            shown = CLIP_HIGHLIGHT;
        }} else if ((params.flags & FLAG_CLIP_SHADOWS) != 0u && any(q == vec3<u32>(0u))) {{
            shown = CLIP_SHADOW;
        }}
        dst[i] = shown.x | (shown.y << 8u) | (shown.z << 16u) | (255u << 24u);
    }}
    workgroupBarrier();
    for (var k = 0u; k < 4u; k++) {{
        let n = atomicLoad(&local_hist[li * 4u + k]);
        if (n > 0u) {{
            atomicAdd(&hist[li * 4u + k], n);
        }}
    }}
}}
"#,
        bins = HISTOGRAM_BINS,
        shoulder = process1::SHOULDER,
        gamma = process1::TONE_GAMMA,
        mid = process1::TONE_MID,
        luma = vec3(LUMA_REC2020),
        s0 = vec3(m[0]),
        s1 = vec3(m[1]),
        s2 = vec3(m[2]),
        flag_tone = FLAG_TONE,
        flag_hi = FLAG_CLIP_HIGHLIGHTS,
        flag_lo = FLAG_CLIP_SHADOWS,
        clip_hi = rgb_u32(CLIP_HIGHLIGHT_RGB),
        clip_lo = rgb_u32(CLIP_SHADOW_RGB),
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
    hist: wgpu::Buffer,
    readback: wgpu::Buffer,
    hist_readback: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

fn wgpu_err(e: impl std::fmt::Display) -> GpuError {
    GpuError::Wgpu(e.to_string())
}

/// Bloc de paramètres, dans la disposition de `Params` (alignement uniforme).
fn uniform_bytes(p: &RenderParams, width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(UNIFORM_SIZE as usize);
    let mut f = |v: f32| out.extend_from_slice(&v.to_le_bytes());
    for row in p.matrix {
        f(row[0]);
        f(row[1]);
        f(row[2]);
        f(0.0);
    }
    for v in p.tone[..4].iter() {
        f(*v);
    }
    f(p.tone[4]);
    f(p.tone[5]);
    f(p.saturation);
    f(p.vibrance);
    for v in [width, height, p.flags, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
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
        let buffer = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = |read_only| wgpu::BufferBindingType::Storage { read_only };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("process1"),
            entries: &[
                buffer(0, storage(true)),
                buffer(1, storage(false)),
                buffer(2, wgpu::BufferBindingType::Uniform),
                buffer(3, storage(false)),
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
        let buffer = |label, size, usage| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        let dst = buffer("rendu", pixels * 4, U::STORAGE | U::COPY_SRC);
        let readback = buffer("lecture", pixels * 4, U::MAP_READ | U::COPY_DST);
        let params = buffer("paramètres", UNIFORM_SIZE, U::UNIFORM | U::COPY_DST);
        let hist = buffer(
            "histogramme",
            HISTOGRAM_BYTES,
            U::STORAGE | U::COPY_SRC | U::COPY_DST,
        );
        let hist_readback = buffer(
            "lecture de l'histogramme",
            HISTOGRAM_BYTES,
            U::MAP_READ | U::COPY_DST,
        );
        fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        }
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("process1"),
            layout: &self.layout,
            entries: &[
                entry(0, &src),
                entry(1, &dst),
                entry(2, &params),
                entry(3, &hist),
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
            hist,
            readback,
            hist_readback,
            bind_group,
        })
    }

    /// Rendu RGBA 8 bits et histogramme, identiques au rendu processeur à la
    /// tolérance près.
    pub fn render(&self, image: &GpuImage, params: &RenderParams) -> Result<Rendered, GpuError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.queue.write_buffer(
            &image.params,
            0,
            &uniform_bytes(params, image.width, image.height),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rendu"),
            });
        encoder.clear_buffer(&image.hist, 0, None);
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
        encoder.copy_buffer_to_buffer(&image.hist, 0, &image.hist_readback, 0, HISTOGRAM_BYTES);
        self.queue.submit([encoder.finish()]);

        let (tx, rx) = mpsc::channel();
        for buffer in [&image.readback, &image.hist_readback] {
            let tx = tx.clone();
            buffer.map_async(wgpu::MapMode::Read, .., move |r| {
                let _ = tx.send(r);
            });
        }
        drop(tx);
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(wgpu_err)?;
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(wgpu_err(e));
        }
        for _ in 0..2 {
            rx.recv().map_err(wgpu_err)?.map_err(wgpu_err)?;
        }
        let rgba = image
            .readback
            .get_mapped_range(..)
            .map_err(wgpu_err)?
            .to_vec();
        image.readback.unmap();
        let histogram = image
            .hist_readback
            .get_mapped_range(..)
            .map_err(wgpu_err)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        image.hist_readback.unmap();
        Ok(Rendered { rgba, histogram })
    }
}
