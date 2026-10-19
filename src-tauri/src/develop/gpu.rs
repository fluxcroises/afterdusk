//! Jumeau GPU du pipeline (décision 0005) : le même calcul que
//! [`super::pipeline::render`], en WGSL par wgpu (Direct3D 12, Metal,
//! Vulkan). Le shader rend les pixels et l'histogramme ; les avertissements
//! d'écrêtage, rares, sont peints ensuite au processeur. Les constantes du shader sont tirées du code processeur, qui reste
//! la référence ; sans processeur graphique utilisable, l'appelant revient au
//! rendu processeur.

use std::sync::{Arc, mpsc};

use wgpu::util::DeviceExt;

use super::curve;
use super::pipeline::{
    FLAG_CURVE, FLAG_GRAIN, FLAG_HSL, FLAG_TONE, FLAG_VIGNETTE, HISTOGRAM_BINS, HUE_CENTERS,
    LUMA_REC2020, REC2020_TO_SRGB, RenderParams, Rendered, WorkingImage, histogram_of,
    paint_clipping, process1,
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
const UNIFORM_SIZE: u64 = 224;

/// Tables des quatre courbes (RVB, rouge, vert, bleu).
const CURVE_BYTES: u64 = 4 * curve::LUT_SIZE as u64 * 4;

const HISTOGRAM_BYTES: u64 = HISTOGRAM_BINS as u64 * 4;

fn vec3(v: [f32; 3]) -> String {
    format!("vec3<f32>({:?}, {:?}, {:?})", v[0], v[1], v[2])
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
    hsl: array<vec4<f32>, 6>,
    vignette: vec4<f32>,
    grain: vec4<f32>,
}}

@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(4) var<storage, read> curve: array<f32>;
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

const FLAG_CURVE: u32 = {flag_curve}u;
const FLAG_HSL: u32 = {flag_hsl}u;
const FLAG_VIGNETTE: u32 = {flag_vignette}u;
const FLAG_GRAIN: u32 = {flag_grain}u;
const HUE_CENTERS = array<f32, 8>({hue_centers});
const LUT_SIZE: u32 = {lut}u;

fn smoothstep_(e0: f32, e1: f32, x: f32) -> f32 {{
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}}

fn vignette_factor(x: u32, y: u32) -> f32 {{
    let u = (f32(x) + 0.5) / f32(params.width) * 2.0 - 1.0;
    let w = (f32(y) + 0.5) / f32(params.height) * 2.0 - 1.0;
    let r = sqrt((u * u + w * w) * 0.5);
    let v = params.vignette;
    let m = smoothstep_(v.y - v.z * 0.5, v.y + v.z * 0.5, r);
    return 1.0 + v.x * m;
}}

fn hsl_adj(k: u32, i: u32) -> f32 {{
    return params.hsl[k * 2u + i / 4u][i % 4u];
}}

fn apply_hsl(c: vec3<f32>) -> vec3<f32> {{
    let mx = max(c.x, max(c.y, c.z));
    let mn = min(c.x, min(c.y, c.z));
    let d = mx - mn;
    if (d <= 1e-6 || mx <= 0.0) {{
        return c;
    }}
    let s = d / mx;
    var h: f32;
    if (mx == c.x) {{
        h = 60.0 * (c.y - c.z) / d;
    }} else if (mx == c.y) {{
        h = 60.0 * ((c.z - c.x) / d + 2.0);
    }} else {{
        h = 60.0 * ((c.x - c.y) / d + 4.0);
    }}
    if (h < 0.0) {{
        h = h + 360.0;
    }}
    var centers = HUE_CENTERS;
    var seg = 7u;
    for (var i = 0u; i < 7u; i++) {{
        if (h < centers[i + 1u]) {{
            seg = i;
            break;
        }}
    }}
    let c0 = centers[seg];
    var c1 = 360.0;
    if (seg < 7u) {{
        c1 = centers[seg + 1u];
    }}
    let t = smoothstep_(0.0, 1.0, (h - c0) / (c1 - c0));
    let nxt = (seg + 1u) % 8u;
    let a_hue = hsl_adj(0u, seg) * (1.0 - t) + hsl_adj(0u, nxt) * t;
    let a_sat = hsl_adj(1u, seg) * (1.0 - t) + hsl_adj(1u, nxt) * t;
    let a_lum = hsl_adj(2u, seg) * (1.0 - t) + hsl_adj(2u, nxt) * t;
    var h2 = h + a_hue;
    if (h2 < 0.0) {{
        h2 = h2 + 360.0;
    }}
    if (h2 >= 360.0) {{
        h2 = h2 - 360.0;
    }}
    let s2 = clamp(s * (1.0 + a_sat), 0.0, 1.0);
    let v2 = clamp(mx * (1.0 + a_lum * s), 0.0, 1.0);
    let chroma = v2 * s2;
    let hh = h2 / 60.0;
    let x = chroma * (1.0 - abs((hh % 2.0) - 1.0));
    let m = v2 - chroma;
    var rgb: vec3<f32>;
    switch u32(hh) {{
        case 0u: {{ rgb = vec3<f32>(chroma, x, 0.0); }}
        case 1u: {{ rgb = vec3<f32>(x, chroma, 0.0); }}
        case 2u: {{ rgb = vec3<f32>(0.0, chroma, x); }}
        case 3u: {{ rgb = vec3<f32>(0.0, x, chroma); }}
        case 4u: {{ rgb = vec3<f32>(x, 0.0, chroma); }}
        default: {{ rgb = vec3<f32>(chroma, 0.0, x); }}
    }}
    return rgb + vec3<f32>(m);
}}

fn lut(k: u32, v: f32) -> f32 {{
    let pos = clamp(v, 0.0, 1.0) * f32(LUT_SIZE - 1u);
    let i = min(u32(pos), LUT_SIZE - 2u);
    let f = pos - f32(i);
    let base = k * LUT_SIZE;
    return curve[base + i] * (1.0 - f) + curve[base + i + 1u] * f;
}}

fn grain_hash(ix: i32, iy: i32, seed: u32) -> f32 {{
    var h = (bitcast<u32>(ix) * 0x8da6b343u) ^ (bitcast<u32>(iy) * 0xd8163841u) ^ (seed * 0xcb1ab31fu);
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {{
    let fx = floor(x);
    let fy = floor(y);
    let ix = i32(fx);
    let iy = i32(fy);
    let tx = x - fx;
    let ty = y - fy;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let a = grain_hash(ix, iy, seed);
    let b = grain_hash(ix + 1, iy, seed);
    let c = grain_hash(ix, iy + 1, seed);
    let d = grain_hash(ix + 1, iy + 1, seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    return top + (bottom - top) * sy;
}}

fn apply_grain(c: vec3<f32>, x: u32, y: u32) -> vec3<f32> {{
    let g = params.grain;
    let cell = g.y;
    let gx = (f32(x) + 0.5) / g.w / cell;
    let gy = (f32(y) + 0.5) / g.w / cell;
    let fine = value_noise(gx * 2.03, gy * 2.03, 1u);
    let n = value_noise(gx, gy, 0u) * (1.0 - g.z) + fine * g.z;
    let attenuation = min(cell * g.w, 1.0);
    let l = 0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z;
    let weight = 0.25 + 3.0 * l * (1.0 - l);
    let delta = (n - 0.5) * g.x * attenuation * weight;
    return c + vec3<f32>(delta);
}}

fn render_pixel(rgb: vec3<f32>, px: u32, py: u32) -> vec3<f32> {{
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
    if ((params.flags & FLAG_VIGNETTE) != 0u) {{
        v = v * vignette_factor(px, py);
    }}
    let srgb = vec3<f32>(dot(S0, v), dot(S1, v), dot(S2, v));
    var out = vec3<f32>(srgb_encode(srgb.x), srgb_encode(srgb.y), srgb_encode(srgb.z));
    if ((params.flags & FLAG_HSL) != 0u) {{
        out = apply_hsl(out);
    }}
    if ((params.flags & FLAG_CURVE) != 0u) {{
        out = vec3<f32>(lut(1u, lut(0u, out.x)), lut(2u, lut(0u, out.y)), lut(3u, lut(0u, out.z)));
    }}
    if ((params.flags & FLAG_GRAIN) != 0u) {{
        out = apply_grain(out, px, py);
    }}
    return out;
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
        let v = render_pixel(vec3<f32>(src[i * 3u], src[i * 3u + 1u], src[i * 3u + 2u]), id.x, id.y);
        let q = vec3<u32>(quantize(v.x), quantize(v.y), quantize(v.z));
        atomicAdd(&local_hist[q.x], 1u);
        atomicAdd(&local_hist[256u + q.y], 1u);
        atomicAdd(&local_hist[512u + q.z], 1u);
        let l = (2126u * q.x + 7152u * q.y + 722u * q.z + 5000u) / 10000u;
        atomicAdd(&local_hist[768u + min(l, 255u)], 1u);
        dst[i] = q.x | (q.y << 8u) | (q.z << 16u) | (255u << 24u);
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
        flag_curve = FLAG_CURVE,
        flag_hsl = FLAG_HSL,
        flag_vignette = FLAG_VIGNETTE,
        flag_grain = FLAG_GRAIN,
        hue_centers = HUE_CENTERS
            .iter()
            .map(|c| format!("{c:?}"))
            .collect::<Vec<_>>()
            .join(", "),
        lut = curve::LUT_SIZE,
        shoulder = process1::SHOULDER,
        gamma = process1::TONE_GAMMA,
        mid = process1::TONE_MID,
        luma = vec3(LUMA_REC2020),
        s0 = vec3(m[0]),
        s1 = vec3(m[1]),
        s2 = vec3(m[2]),
        flag_tone = FLAG_TONE,
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
    curves: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    scale: f32,
}

fn wgpu_err(e: impl std::fmt::Display) -> GpuError {
    GpuError::Wgpu(e.to_string())
}

/// Bloc de paramètres, dans la disposition de `Params` (alignement uniforme).
fn uniform_bytes(p: &RenderParams, width: u32, height: u32, scale: f32) -> Vec<u8> {
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
    let mut f = |v: f32| out.extend_from_slice(&v.to_le_bytes());
    for set in &p.hsl {
        for v in set {
            f(*v);
        }
    }
    for v in p.vignette {
        f(v);
    }
    f(0.0);
    for v in p.grain {
        f(v);
    }
    f(scale);
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
                buffer(4, storage(true)),
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
        let curves = buffer("courbes", CURVE_BYTES, U::STORAGE | U::COPY_DST);
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
                entry(4, &curves),
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
            curves,
            bind_group,
            scale: image.scale,
        })
    }

    /// Rendu RGBA 8 bits et histogramme, identiques au rendu processeur à la
    /// tolérance près.
    pub fn render(&self, image: &GpuImage, params: &RenderParams) -> Result<Rendered, GpuError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.queue.write_buffer(
            &image.params,
            0,
            &uniform_bytes(params, image.width, image.height, image.scale),
        );
        if params.flags & FLAG_CURVE != 0 {
            let bytes: Vec<u8> = params.curve.iter().flat_map(|v| v.to_le_bytes()).collect();
            self.queue.write_buffer(&image.curves, 0, &bytes);
        }
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
        let mut rgba = image
            .readback
            .get_mapped_range(..)
            .map_err(wgpu_err)?
            .to_vec();
        image.readback.unmap();
        let mut histogram: Vec<u32> = image
            .hist_readback
            .get_mapped_range(..)
            .map_err(wgpu_err)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        image.hist_readback.unmap();
        // Histogramme incohérent (constaté sur le processeur graphique virtuel
        // des machines d'intégration Mac) : recalculé au processeur.
        let pixels = u64::from(image.width) * u64::from(image.height);
        let consistent = histogram
            .chunks(256)
            .all(|c| c.iter().map(|v| u64::from(*v)).sum::<u64>() == pixels);
        if !consistent {
            histogram = histogram_of(&rgba);
        }
        // Avertissements d'écrêtage peints après coup, d'après les pixels rendus.
        paint_clipping(&mut rgba, params.flags);
        Ok(Rendered { rgba, histogram })
    }
}
