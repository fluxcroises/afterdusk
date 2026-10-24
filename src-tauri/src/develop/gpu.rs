//! Jumeau GPU du pipeline (décision 0005) : le même calcul que
//! [`super::pipeline::render`], en WGSL par wgpu (Direct3D 12, Metal,
//! Vulkan). Le shader rend les pixels et l'histogramme ; les avertissements
//! d'écrêtage, rares, sont peints ensuite au processeur. Les constantes du shader sont tirées du code processeur, qui reste
//! la référence ; sans processeur graphique utilisable, l'appelant revient au
//! rendu processeur.

use std::sync::{Arc, mpsc};

use wgpu::util::DeviceExt;

use super::curve;
use super::detail::{self, Radii};
use super::pipeline::{
    FLAG_CURVE, FLAG_DENOISE, FLAG_GRAIN, FLAG_HSL, FLAG_MASKS, FLAG_RENDERED, FLAG_SHARPEN,
    FLAG_TONE, FLAG_VIGNETTE, HISTOGRAM_BINS, HUE_CENTERS, LUMA_REC2020, REC2020_TO_SRGB,
    RenderParams, Rendered, WorkingImage, histogram_of, paint_clipping, process1,
};
use super::settings::MASKS_MAX;

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
const UNIFORM_SIZE: u64 = 288;

/// Tables des quatre courbes (RVB, rouge, vert, bleu).
const CURVE_BYTES: u64 = 4 * curve::LUT_SIZE as u64 * 4;

const HISTOGRAM_BYTES: u64 = HISTOGRAM_BINS as u64 * 4;

/// Valeurs par masque dans le tampon des retouches locales (voir
/// [`mask_bytes`]).
const MASK_STRIDE: usize = 20;
const MASK_BYTES: u64 = (MASKS_MAX * MASK_STRIDE * 4) as u64;

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
    width: u32, height: u32, flags: u32, masks: u32,
    hsl: array<vec4<f32>, 6>,
    vignette: vec4<f32>,
    grain: vec4<f32>,
    frame: vec4<u32>,
    detail_a: vec4<f32>,
    detail_b: vec4<f32>,
    radii: vec4<u32>,
}}

@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(4) var<storage, read> curve: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> hist: array<atomic<u32>, {bins}>;
@group(0) @binding(8) var<storage, read> mask_data: array<f32>;

var<workgroup> local_hist: array<atomic<u32>, {bins}>;

const SHOULDER: f32 = {shoulder:?};
const TONE_GAMMA: f32 = {gamma:?};
const TONE_MID: f32 = {mid:?};
const LUMA = {luma};
const S0 = {s0};
const S1 = {s1};
const S2 = {s2};
const FLAG_TONE: u32 = {flag_tone}u;
const FLAG_RENDERED: u32 = {flag_rendered}u;
const MASKS_MAX: u32 = {masks_max}u;
const MASK_STRIDE: u32 = {mask_stride}u;

fn neutral_curve(x: f32) -> f32 {{
    // Source déjà développée : identité bornée, pas d'épaule.
    if ((params.flags & FLAG_RENDERED) != 0u) {{
        return clamp(x, 0.0, 1.0);
    }}
    if (x <= SHOULDER) {{
        return max(x, 0.0);
    }}
    return SHOULDER + (1.0 - SHOULDER) * (1.0 - exp(-(x - SHOULDER) / (1.0 - SHOULDER)));
}}

fn tone_curve(p: f32) -> f32 {{
    return tone_curve_with(p, params.tone_a, params.tone_b.xy);
}}

fn tone_curve_with(p: f32, a: vec4<f32>, b: vec2<f32>) -> f32 {{
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
    let u = (f32(x) + 0.5) / f32(params.frame.z) * 2.0 - 1.0;
    let w = (f32(y) + 0.5) / f32(params.frame.w) * 2.0 - 1.0;
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
    var out = display_pixel(lin, px, py);
    if ((params.flags & FLAG_GRAIN) != 0u) {{
        out = apply_grain(out, px, py);
    }}
    return out;
}}

// Poids du masque `i` au centre (x, y) d'un pixel de la sortie entière.
fn mask_weight(i: u32, x: f32, y: f32) -> f32 {{
    let b = i * MASK_STRIDE;
    let u = mask_data[b + 4u] * x + mask_data[b + 5u] * y + mask_data[b + 6u];
    var w = 0.0;
    if (mask_data[b] != 0.0) {{
        let v = mask_data[b + 7u] * x + mask_data[b + 8u] * y + mask_data[b + 9u];
        let q = sqrt(u * u + v * v);
        let inner = mask_data[b + 2u];
        if (q >= 1.0) {{
            w = 0.0;
        }} else if (q <= inner) {{
            w = 1.0;
        }} else {{
            w = 1.0 - smoothstep_(0.0, 1.0, (q - inner) / (1.0 - inner));
        }}
    }} else {{
        w = 1.0 - smoothstep_(0.0, 1.0, u);
    }}
    if (mask_data[b + 1u] != 0.0) {{
        w = 1.0 - w;
    }}
    return w;
}}

fn display_pixel(lin_in: vec3<f32>, px: u32, py: u32) -> vec3<f32> {{
    // Retouches locales : poids de chaque masque en ce pixel, puis gains
    // (exposition, température, teinte) en lumière linéaire.
    let n = min(params.masks, MASKS_MAX);
    let fx = f32(px) + 0.5;
    let fy = f32(py) + 0.5;
    var log_gain = vec3<f32>(0.0);
    var tone_a = vec4<f32>(0.0);
    var tone_b = vec2<f32>(0.0);
    var local_saturation = 0.0;
    for (var i = 0u; i < n; i++) {{
        let w = mask_weight(i, fx, fy);
        if (w <= 0.0) {{
            continue;
        }}
        let b = i * MASK_STRIDE;
        log_gain += w * vec3<f32>(mask_data[b + 10u], mask_data[b + 11u], mask_data[b + 12u]);
        if (mask_data[b + 3u] != 0.0) {{
            tone_a += w * vec4<f32>(mask_data[b + 13u], mask_data[b + 14u], mask_data[b + 15u], mask_data[b + 16u]);
            tone_b += w * vec2<f32>(mask_data[b + 17u], mask_data[b + 18u]);
        }}
        local_saturation += w * mask_data[b + 19u];
    }}
    let local_tone = any(tone_a != vec4<f32>(0.0)) || any(tone_b != vec2<f32>(0.0));
    var lin = lin_in;
    if (any(log_gain != vec3<f32>(0.0))) {{
        lin = lin * exp2(log_gain);
    }}
    var v = vec3<f32>(neutral_curve(lin.x), neutral_curve(lin.y), neutral_curve(lin.z));
    if ((params.flags & FLAG_TONE) != 0u || local_tone) {{
        let y = dot(LUMA, v);
        if (y > 1e-6) {{
            let q = pow(y, 1.0 / TONE_GAMMA);
            var t = q;
            if ((params.flags & FLAG_TONE) != 0u) {{
                t = tone_curve(q);
            }}
            // Courbe des masques, de leurs amplitudes pondérées, après celle
            // de la photo.
            if (local_tone) {{
                t = tone_curve_with(t, tone_a, tone_b);
            }}
            var ratio = 0.0;
            if (t > 0.0) {{
                ratio = pow(t, TONE_GAMMA) / y;
            }}
            v = v * ratio;
        }}
    }}
    let saturation = params.tone_b.z;
    let vibrance = params.tone_b.w;
    if (saturation != 0.0 || vibrance != 0.0 || local_saturation != 0.0) {{
        let y = dot(LUMA, v);
        let mx = max(v.x, max(v.y, v.z));
        let mn = min(v.x, min(v.y, v.z));
        var chroma = 0.0;
        if (mx > 1e-6) {{
            chroma = clamp((mx - mn) / mx, 0.0, 1.0);
        }}
        let dull = 1.0 - chroma;
        let factor = (1.0 + saturation) * (1.0 + vibrance * dull * dull)
            * max(1.0 + local_saturation, 0.0);
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
        let v = render_pixel(vec3<f32>(src[i * 3u], src[i * 3u + 1u], src[i * 3u + 2u]), params.frame.x + id.x, params.frame.y + id.y);
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
        flag_rendered = FLAG_RENDERED,
        masks_max = MASKS_MAX,
        mask_stride = MASK_STRIDE,
        wg = WORKGROUP,
    )
}

/// Passes du rendu par étapes (réduction du bruit, netteté), dans l'ordre.
const STAGED_PASSES: [&str; 16] = [
    "s_linear",
    "s_chroma",
    "s_box_h_color",
    "s_box_v_color",
    "s_chroma_apply",
    "s_guide",
    "s_box_h_lum1",
    "s_box_v_lum1",
    "s_coef",
    "s_box_h_lum2",
    "s_box_v_lum2",
    "s_lum_apply",
    "s_display",
    "s_gauss_h",
    "s_gauss_v",
    "s_finish",
];

/// Moyenne de surface d'un tampon à deux valeurs par pixel, sur une ligne
/// (`horizontal`) ou une colonne, comme `detail::box_blur`.
fn box_pass(name: &str, from: &str, to: &str, radius: &str, horizontal: bool) -> String {
    let (pos, len, at) = if horizontal {
        ("id.x", "params.width", "id.y * params.width + k")
    } else {
        ("id.y", "params.height", "k * params.width + id.x")
    };
    format!(
        r#"
@compute @workgroup_size({wg}, {wg})
fn {name}(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (id.x >= params.width || id.y >= params.height) {{
        return;
    }}
    let r = params.radii.{radius};
    var lo = 0u;
    if ({pos} > r) {{
        lo = {pos} - r;
    }}
    let hi = min({pos} + r, {len} - 1u);
    let n = f32(hi - lo + 1u);
    var s0 = 0.0;
    for (var k = lo; k <= hi; k++) {{
        s0 = s0 + {from}[({at}) * 2u];
    }}
    var s1 = 0.0;
    for (var k = lo; k <= hi; k++) {{
        s1 = s1 + {from}[({at}) * 2u + 1u];
    }}
    let o = (id.y * params.width + id.x) * 2u;
    {to}[o] = s0 / n;
    {to}[o + 1u] = s1 / n;
}}
"#,
        wg = WORKGROUP,
    )
}

/// Point d'entrée avec histogramme par groupe de travail (comme `main`) :
/// `body` calcule `v` (valeurs codées) pour le pixel `i`.
fn histogram_entry(name: &str, body: &str) -> String {
    format!(
        r#"
@compute @workgroup_size({wg}, {wg})
fn {name}(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {{
    for (var k = 0u; k < 4u; k++) {{
        atomicStore(&local_hist[li * 4u + k], 0u);
    }}
    workgroupBarrier();
    if (id.x < params.width && id.y < params.height) {{
        let i = id.y * params.width + id.x;
        let px = params.frame.x + id.x;
        let py = params.frame.y + id.y;
{body}
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
        wg = WORKGROUP,
    )
}

/// Passes de réduction du bruit et de netteté (voir `detail.rs`).
fn staged_source() -> String {
    let mut out = format!(
        r#"
// Trois tampons seulement (la limite garantie est de huit par étape) : l'image
// linéaire ; les passes A, B, C (deux valeurs par pixel) bout à bout ; les
// valeurs codées puis les deux luminances de la netteté.
@group(0) @binding(5) var<storage, read_write> lin: array<f32>;
@group(0) @binding(6) var<storage, read_write> abc: array<f32>;
@group(0) @binding(7) var<storage, read_write> el: array<f32>;

fn npx() -> u32 {{
    return params.width * params.height;
}}

fn ob() -> u32 {{
    return 2u * npx();
}}

fn oc() -> u32 {{
    return 4u * npx();
}}

fn ol() -> u32 {{
    return 3u * npx();
}}

fn ol2() -> u32 {{
    return 4u * npx();
}}

const FLAG_SHARPEN: u32 = {flag_sharpen}u;

fn lin_at(i: u32) -> vec3<f32> {{
    return vec3<f32>(lin[i * 3u], lin[i * 3u + 1u], lin[i * 3u + 2u]);
}}

fn set_lin(i: u32, v: vec3<f32>) {{
    lin[i * 3u] = v.x;
    lin[i * 3u + 1u] = v.y;
    lin[i * 3u + 2u] = v.z;
}}

fn outside(id: vec3<u32>) -> bool {{
    return id.x >= params.width || id.y >= params.height;
}}

@compute @workgroup_size({wg}, {wg})
fn s_linear(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let rgb = vec3<f32>(src[i * 3u], src[i * 3u + 1u], src[i * 3u + 2u]);
    set_lin(i, vec3<f32>(dot(params.m0.xyz, rgb), dot(params.m1.xyz, rgb), dot(params.m2.xyz, rgb)));
}}

@compute @workgroup_size({wg}, {wg})
fn s_chroma(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let c = lin_at(i);
    let y = dot(LUMA, c);
    buf_a[i * 2u] = c.x - y;
    buf_a[i * 2u + 1u] = c.z - y;
}}

@compute @workgroup_size({wg}, {wg})
fn s_chroma_apply(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let c = lin_at(i);
    let y = dot(LUMA, c);
    let amount = params.detail_a.y;
    let cr = buf_a[i * 2u] + (buf_c[i * 2u] - buf_a[i * 2u]) * amount;
    let cb = buf_a[i * 2u + 1u] + (buf_c[i * 2u + 1u] - buf_a[i * 2u + 1u]) * amount;
    let red = y + cr;
    let blue = y + cb;
    set_lin(i, vec3<f32>(red, (y - LUMA.x * red - LUMA.z * blue) / LUMA.y, blue));
}}

@compute @workgroup_size({wg}, {wg})
fn s_guide(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let y = dot(LUMA, lin_at(i));
    var v = 0.0;
    if (y > 0.0) {{
        v = pow(y, 1.0 / TONE_GAMMA);
    }}
    buf_a[i * 2u] = v;
    buf_a[i * 2u + 1u] = v * v;
}}

@compute @workgroup_size({wg}, {wg})
fn s_coef(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let m = buf_c[i * 2u];
    let mm = buf_c[i * 2u + 1u];
    let variance = max(mm - m * m, 0.0);
    let a = variance / (variance + params.detail_a.x);
    buf_b[i * 2u] = a;
    buf_b[i * 2u + 1u] = m * (1.0 - a);
}}

@compute @workgroup_size({wg}, {wg})
fn s_lum_apply(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let c = lin_at(i);
    let y = dot(LUMA, c);
    if (y > 1e-6) {{
        let q = max(buf_b[i * 2u] * buf_a[i * 2u] + buf_b[i * 2u + 1u], 0.0);
        var ratio = 0.0;
        if (q > 0.0) {{
            ratio = pow(q, TONE_GAMMA) / y;
        }}
        set_lin(i, c * ratio);
    }}
}}

@compute @workgroup_size({wg}, {wg})
fn s_display(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let i = id.y * params.width + id.x;
    let e = display_pixel(lin_at(i), params.frame.x + id.x, params.frame.y + id.y);
    enc[i * 3u] = e.x;
    enc[i * 3u + 1u] = e.y;
    enc[i * 3u + 2u] = e.z;
    lum[i] = 0.2126 * e.x + 0.7152 * e.y + 0.0722 * e.z;
}}

fn gauss_weight(k: i32) -> f32 {{
    let sigma = params.detail_b.y;
    return exp(-f32(k * k) / (2.0 * sigma * sigma));
}}

@compute @workgroup_size({wg}, {wg})
fn s_gauss_h(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let kernel = i32(params.radii.z);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -kernel; k <= kernel; k++) {{
        let xx = i32(id.x) + k;
        if (xx >= 0 && xx < i32(params.width)) {{
            let wt = gauss_weight(k);
            sum = sum + lum[id.y * params.width + u32(xx)] * wt;
            total = total + wt;
        }}
    }}
    lum2[id.y * params.width + id.x] = sum / total;
}}

@compute @workgroup_size({wg}, {wg})
fn s_gauss_v(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (outside(id)) {{
        return;
    }}
    let kernel = i32(params.radii.z);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -kernel; k <= kernel; k++) {{
        let yy = i32(id.y) + k;
        if (yy >= 0 && yy < i32(params.height)) {{
            let wt = gauss_weight(k);
            sum = sum + lum2[u32(yy) * params.width + id.x] * wt;
            total = total + wt;
        }}
    }}
    buf_a[(id.y * params.width + id.x) * 2u] = sum / total;
}}

fn blurred(x: u32, y: u32) -> f32 {{
    return buf_a[(y * params.width + x) * 2u];
}}
"#,
        flag_sharpen = FLAG_SHARPEN,
        wg = WORKGROUP,
    );
    out.push_str(&box_pass("s_box_h_color", "buf_a", "buf_b", "y", true));
    out.push_str(&box_pass("s_box_v_color", "buf_b", "buf_c", "y", false));
    out.push_str(&box_pass("s_box_h_lum1", "buf_a", "buf_b", "x", true));
    out.push_str(&box_pass("s_box_v_lum1", "buf_b", "buf_c", "x", false));
    out.push_str(&box_pass("s_box_h_lum2", "buf_b", "buf_c", "x", true));
    out.push_str(&box_pass("s_box_v_lum2", "buf_c", "buf_b", "x", false));
    // Noms des tampons logiques vers leur place dans les trois tampons réels.
    let mut out = out
        .replace("buf_a[", "abc[")
        .replace("buf_b[", "abc[ob() + ")
        .replace("buf_c[", "abc[oc() + ")
        .replace("lum2[", "el[ol2() + ")
        .replace("lum[", "el[ol() + ")
        .replace("enc[", "el[");
    out.push_str(&histogram_entry(
        "s_finish",
        r#"        var v = vec3<f32>(enc[i * 3u], enc[i * 3u + 1u], enc[i * 3u + 2u]);
        if ((params.flags & FLAG_SHARPEN) != 0u) {
            let d = lum[i] - blurred(id.x, id.y);
            let soft = d / (1.0 + abs(d) / params.detail_a.w);
            var mask = 1.0;
            if (params.detail_b.z > 0.0) {
                let edge = params.detail_b.x;
                let xl = select(id.x - 1u, 0u, id.x == 0u);
                let xr = min(id.x + 1u, params.width - 1u);
                let yt = select(id.y - 1u, 0u, id.y == 0u);
                let yb = min(id.y + 1u, params.height - 1u);
                let gx = blurred(xr, id.y) - blurred(xl, id.y);
                let gy = blurred(id.x, yb) - blurred(id.x, yt);
                mask = smoothstep_(0.5 * edge, 1.5 * edge, 0.5 * sqrt(gx * gx + gy * gy));
            }
            v = v + vec3<f32>(params.detail_a.z * soft * mask);
        }
        if ((params.flags & FLAG_GRAIN) != 0u) {
            v = apply_grain(v, px, py);
        }"#
        .replace("lum[", "el[ol() + ")
        .replace("enc[", "el[")
        .as_str(),
    ));
    out
}

/// Processeur graphique prêt à rendre : appareil, file et pipeline compilé.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// Passes de voisinage, dans l'ordre de [`STAGED_PASSES`].
    staged: Vec<wgpu::ComputePipeline>,
    staged_layout: wgpu::BindGroupLayout,
    adapter: String,
}

/// Tampons des passes de voisinage, créés au premier rendu qui en a besoin.
struct Staged {
    _buffers: Vec<wgpu::Buffer>,
    bind_group: wgpu::BindGroup,
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
    /// Retouches locales (voir [`mask_bytes`]).
    masks: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    src: wgpu::Buffer,
    staged: std::sync::OnceLock<Staged>,
    scale: f32,
    /// Origine dans l'image entière et taille de celle-ci (voir [`WorkingImage::frame`]).
    frame: (u32, u32, u32, u32),
}

fn wgpu_err(e: impl std::fmt::Display) -> GpuError {
    GpuError::Wgpu(e.to_string())
}

/// Bloc de paramètres, dans la disposition de `Params` (alignement uniforme).
fn uniform_bytes(
    p: &RenderParams,
    width: u32,
    height: u32,
    scale: f32,
    frame: (u32, u32, u32, u32),
) -> Vec<u8> {
    let radii = Radii::at(&p.detail, scale);
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
    for v in [width, height, p.flags, mask_count(p)] {
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
    for v in [frame.0, frame.1, frame.2, frame.3] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let d = &p.detail;
    let mut f = |v: f32| out.extend_from_slice(&v.to_le_bytes());
    for v in [d.eps(), d.color, d.sharpen, d.sharpen_limit()] {
        f(v);
    }
    let edge = detail::process1::SHARPEN_MASK_EDGE * d.masking;
    for v in [edge, radii.sigma, f32::from(u8::from(d.masking > 0.0)), 0.0] {
        f(v);
    }
    for v in [radii.luminance, radii.color, radii.kernel, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Nombre de masques actifs envoyés au shader.
fn mask_count(p: &RenderParams) -> u32 {
    match p.locals.as_deref() {
        Some(l) if p.flags & FLAG_MASKS != 0 => l.params.len().min(MASKS_MAX) as u32,
        _ => 0,
    }
}

/// Masques des retouches locales, `MASK_STRIDE` valeurs chacun, lues par
/// `mask_weight` et `display_pixel` : forme ramenée en pixels de la sortie
/// (radial, inversé, bord intérieur, puis `u` et `v`), gains, courbe de
/// tonalité et saturation.
fn mask_bytes(p: &RenderParams) -> Vec<u8> {
    let Some(l) = p.locals.as_deref() else {
        return Vec::new();
    };
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let mut out = Vec::with_capacity(MASK_BYTES as usize);
    let n = mask_count(p) as usize;
    for (shape, lp) in l.planar.iter().zip(&l.params).take(n) {
        let mut values = vec![
            flag(shape.radial),
            flag(shape.invert),
            shape.inner as f32,
            flag(lp.has_tone),
        ];
        values.extend(shape.u.iter().chain(&shape.v).map(|v| *v as f32));
        values.extend(lp.log_gain);
        values.extend(lp.tone);
        values.push(lp.saturation);
        debug_assert_eq!(values.len(), MASK_STRIDE);
        out.extend(values.iter().flat_map(|v| v.to_le_bytes()));
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
            source: wgpu::ShaderSource::Wgsl((shader_source() + &staged_source()).into()),
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
                buffer(8, storage(true)),
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
        let mut staged_entries: Vec<wgpu::BindGroupLayoutEntry> = vec![
            buffer(0, storage(true)),
            buffer(1, storage(false)),
            buffer(2, wgpu::BufferBindingType::Uniform),
            buffer(3, storage(false)),
            buffer(4, storage(true)),
            buffer(8, storage(true)),
        ];
        staged_entries.extend((5..8).map(|b| buffer(b, storage(false))));
        let staged_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("process1-passes"),
            entries: &staged_entries,
        });
        let staged_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("process1-passes"),
                bind_group_layouts: &[Some(&staged_layout)],
                immediate_size: 0,
            });
        let staged = STAGED_PASSES
            .iter()
            .map(|name| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(name),
                    layout: Some(&staged_pipeline_layout),
                    module: &module,
                    entry_point: Some(name),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .collect();
        if let Some(e) = scope.pop().await {
            return Err(wgpu_err(e));
        }
        Ok(Self {
            staged_layout,
            staged,
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
        let masks = buffer("masques", MASK_BYTES, U::STORAGE | U::COPY_DST);
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
                entry(8, &masks),
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
            masks,
            bind_group,
            src,
            staged: std::sync::OnceLock::new(),
            scale: image.scale,
            frame: image.frame(),
        })
    }

    /// Tampons et liaisons des passes de voisinage d'une image.
    fn staged_buffers(&self, image: &GpuImage) -> Staged {
        let pixels = u64::from(image.width) * u64::from(image.height);
        let make = |label: &str, per_pixel: u64| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: pixels * per_pixel * 4,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        };
        let buffers = vec![
            make("linéaire", 3),
            make("passes A, B, C", 6),
            make("codé et luminances", 5),
        ];
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: image.src.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: image.dst.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: image.params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: image.hist.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: image.curves.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: image.masks.as_entire_binding(),
            },
        ];
        for (k, b) in buffers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: 5 + k as u32,
                resource: b.as_entire_binding(),
            });
        }
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("process1-passes"),
            layout: &self.staged_layout,
            entries: &entries,
        });
        Staged {
            _buffers: buffers,
            bind_group,
        }
    }

    /// Rendu RGBA 8 bits et histogramme, identiques au rendu processeur à la
    /// tolérance près.
    pub fn render(&self, image: &GpuImage, params: &RenderParams) -> Result<Rendered, GpuError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.queue.write_buffer(
            &image.params,
            0,
            &uniform_bytes(params, image.width, image.height, image.scale, image.frame),
        );
        if params.flags & FLAG_CURVE != 0 {
            let bytes: Vec<u8> = params.curve.iter().flat_map(|v| v.to_le_bytes()).collect();
            self.queue.write_buffer(&image.curves, 0, &bytes);
        }
        if mask_count(params) > 0 {
            self.queue
                .write_buffer(&image.masks, 0, &mask_bytes(params));
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rendu"),
            });
        encoder.clear_buffer(&image.hist, 0, None);
        let groups = (
            image.width.div_ceil(WORKGROUP),
            image.height.div_ceil(WORKGROUP),
        );
        if params.flags & (FLAG_DENOISE | FLAG_SHARPEN) != 0 {
            let staged = image.staged.get_or_init(|| self.staged_buffers(image));
            let wanted = |name: &str| match name {
                "s_chroma" | "s_box_h_color" | "s_box_v_color" | "s_chroma_apply" => {
                    params.detail.color > 0.0
                }
                "s_guide" | "s_box_h_lum1" | "s_box_v_lum1" | "s_coef" | "s_box_h_lum2"
                | "s_box_v_lum2" | "s_lum_apply" => params.detail.luminance > 0.0,
                "s_gauss_h" | "s_gauss_v" => params.detail.sharpens(),
                _ => true,
            };
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("process1-passes"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &staged.bind_group, &[]);
            for (name, pipeline) in STAGED_PASSES.iter().zip(&self.staged) {
                if wanted(name) {
                    pass.set_pipeline(pipeline);
                    pass.dispatch_workgroups(groups.0, groups.1, 1);
                }
            }
        } else {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("process1"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &image.bind_group, &[]);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
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
