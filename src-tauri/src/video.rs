//! Vidéos (cahier des charges, 4.9 ; décision 0012) : ffprobe lit les
//! informations, ffmpeg calcule les miniatures et, quand le lecteur intégré
//! ne sait pas lire l'original, une version de lecture H.264. Ce sont des
//! programmes séparés livrés avec Afterdusk ; l'original n'est qu'ouvert en
//! lecture.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use chrono::{DateTime, Local, Utc};
use image::RgbImage;
use serde::Serialize;
use serde_json::Value;

/// Extensions vidéo reconnues (en minuscules).
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mts", "m2ts", "avi", "mxf"];

/// Fichiers annexes des boîtiers, rattachés à leur vidéo (en minuscules) :
/// métadonnées Sony (`C0001M01.XML`), vignettes Canon et GoPro (`.THM`),
/// basses définitions GoPro et DJI (`.LRV`, `.LRF`), sous-titres DJI (`.SRT`).
pub const ANNEX_EXTENSIONS: &[&str] = &["xml", "thm", "lrv", "lrf", "srt"];

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Dossier de ffmpeg et ffprobe, donné au démarrage par l'application.
pub fn set_dir(path: PathBuf) {
    let _ = DIR.set(path);
}

fn tool(name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    DIR.get()
        .map(|d| d.join(&file))
        .filter(|p| p.is_file())
        .or_else(|| {
            // Tests et développement : la copie récupérée par le script.
            let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("ffmpeg")
                .join(&file);
            dev.is_file().then_some(dev)
        })
}

/// FFmpeg est-il là ? Sans lui, les vidéos s'importent sans informations.
pub fn available() -> bool {
    tool("ffprobe").is_some() && tool("ffmpeg").is_some()
}

fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Pas de fenêtre de console à chaque appel.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub duration_ms: i64,
    /// Dimensions affichées (rotation du boîtier comprise).
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// « h264 », « hevc », « prores »…
    pub codec: String,
    pub has_audio: bool,
    /// `AAAA-MM-JJTHH:MM:SS`, heure locale de l'ordinateur.
    pub captured_at: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
}

fn ratio(text: &str) -> Option<f64> {
    let (a, b) = text.split_once('/')?;
    let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
    (b > 0.0 && a > 0.0).then(|| a / b)
}

/// Date d'une balise `creation_time` (UTC dans les MP4 et MOV), en heure locale.
fn local_time(text: &str) -> Option<String> {
    let utc: DateTime<Utc> = text.parse().ok()?;
    Some(
        utc.with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
    )
}

/// Informations d'une vidéo, lues par ffprobe.
pub fn probe(path: &Path) -> Option<VideoInfo> {
    let output = command(&tool("ffprobe")?)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: Value = serde_json::from_slice(&output.stdout).ok()?;
    let streams = json["streams"].as_array()?;
    let video = streams.iter().find(|s| s["codec_type"] == "video")?;
    let format = &json["format"];
    let tags = &format["tags"];
    let text = |v: &Value| v.as_str().map(str::to_owned);
    let mut width = video["width"].as_u64().unwrap_or(0) as u32;
    let mut height = video["height"].as_u64().unwrap_or(0) as u32;
    // Vidéo tournée par le boîtier : la matrice d'affichage le dit.
    let rotation = video["side_data_list"]
        .as_array()
        .and_then(|l| l.iter().find_map(|d| d["rotation"].as_f64()))
        .or_else(|| {
            video["tags"]["rotate"]
                .as_str()
                .and_then(|r| r.parse().ok())
        })
        .unwrap_or(0.0);
    if (rotation.abs() - 90.0).abs() < 1.0 {
        std::mem::swap(&mut width, &mut height);
    }
    let duration = format["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| video["duration"].as_str().and_then(|d| d.parse().ok()))
        .unwrap_or(0.0);
    Some(VideoInfo {
        duration_ms: (duration * 1000.0).round() as i64,
        width,
        height,
        fps: video["avg_frame_rate"]
            .as_str()
            .and_then(ratio)
            .or_else(|| video["r_frame_rate"].as_str().and_then(ratio))
            .unwrap_or(0.0),
        codec: video["codec_name"].as_str().unwrap_or_default().to_owned(),
        has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        captured_at: tags["creation_time"]
            .as_str()
            .or_else(|| video["tags"]["creation_time"].as_str())
            .and_then(local_time),
        make: text(&tags["com.apple.quicktime.make"]).or_else(|| text(&tags["make"])),
        model: text(&tags["com.apple.quicktime.model"]).or_else(|| text(&tags["model"])),
    })
}

/// Image de la vidéo à `at` secondes, réduite à `max_side` (côté long).
pub fn frame(path: &Path, at: f64, max_side: u32) -> Option<RgbImage> {
    let scale = format!(
        "scale='if(gt(iw,ih),min({max_side},iw),-2)':'if(gt(iw,ih),-2,min({max_side},ih))'"
    );
    let output = command(&tool("ffmpeg")?)
        .args(["-v", "error", "-ss", &format!("{at:.3}"), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &scale,
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "-",
        ])
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    image::load_from_memory(&output.stdout)
        .ok()
        .map(|i| i.to_rgb8())
}

/// Image fixe HEIF (HEIC des téléphones, mosaïques comprises) décodée par
/// FFmpeg en PNG 16 bits, profil ICC conservé. L'orientation n'est pas
/// appliquée : Afterdusk l'applique d'après l'EXIF, comme pour un JPEG.
pub fn still(path: &Path) -> Option<Vec<u8>> {
    let output = command(&tool("ffmpeg")?)
        .args(["-v", "error", "-noautorotate", "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-pix_fmt",
            "rgb48be",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "-",
        ])
        .output()
        .ok()?;
    (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
}

/// Miniature : une image prise à 10 % de la durée (pas l'écran noir du début).
pub fn thumbnail(path: &Path, max_side: u32) -> Option<RgbImage> {
    let at = probe(path).map_or(0.0, |i| i.duration_ms as f64 / 10_000.0);
    frame(path, at, max_side).or_else(|| frame(path, 0.0, max_side))
}

/// Codecs que les lecteurs intégrés des deux systèmes lisent sans extension.
pub fn plays_natively(codec: &str) -> bool {
    // HEVC : natif sous macOS, extension Microsoft payante sous Windows.
    codec == "h264" || (cfg!(target_os = "macos") && codec == "hevc")
}

/// Version de lecture H.264 (1080p au plus) de la vidéo, écrite d'un coup
/// à `out` : un fichier temporaire est renommé à la fin.
pub fn proxy(path: &Path, out: &Path) -> bool {
    let Some(ffmpeg) = tool("ffmpeg") else {
        return false;
    };
    let tmp = out.with_extension("tmp.mp4");
    let ok = command(&ffmpeg)
        .args(["-v", "error", "-y", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            "scale=-2:'min(1080,ih)'",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
        ])
        .arg(&tmp)
        .status()
        .is_ok_and(|s| s.success());
    ok && std::fs::rename(&tmp, out).is_ok()
}

/// Instants des images clés de la première piste vidéo, en millisecondes,
/// dans l'ordre. Lus dans les paquets, sans décoder : rapide même sur un
/// long clip. Une coupe sans réencodage ne peut commencer que sur l'une
/// d'elles.
pub fn keyframes(path: &Path) -> Option<Vec<u64>> {
    let ffprobe = tool("ffprobe")?;
    let out = command(&ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "packet=pts_time,flags",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut times: Vec<u64> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let (time, flags) = line.split_once(',')?;
            flags.contains('K').then_some(())?;
            let t: f64 = time.trim().parse().ok()?;
            (t.is_finite() && t >= 0.0).then(|| (t * 1000.0).round() as u64)
        })
        .collect();
    times.sort_unstable();
    times.dedup();
    Some(times)
}

/// Image clé au plus tard à `at_ms` (0 s'il n'y en a pas avant).
pub fn snap(keyframes: &[u64], at_ms: u64) -> u64 {
    keyframes
        .iter()
        .copied()
        .take_while(|&k| k <= at_ms)
        .last()
        .unwrap_or(0)
}

/// Extrait de `start_ms` à `end_ms` copié sans réencodage (vidéo et son)
/// dans `out`, par un fichier temporaire renommé à la fin. `start_ms` doit
/// tomber sur une image clé (`snap`) pour que l'extrait commence là.
pub fn cut(path: &Path, start_ms: u64, end_ms: u64, out: &Path) -> bool {
    let Some(ffmpeg) = tool("ffmpeg") else {
        return false;
    };
    if end_ms <= start_ms {
        return false;
    }
    let tmp = temporary(out);
    let seconds = |ms: u64| format!("{:.3}", ms as f64 / 1000.0);
    let ok = command(&ffmpeg)
        .args(["-v", "error", "-y", "-ss", &seconds(start_ms), "-i"])
        .arg(path)
        .args([
            "-t",
            &seconds(end_ms - start_ms),
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c",
            "copy",
            "-map_metadata",
            "0",
            "-avoid_negative_ts",
            "make_zero",
        ])
        .arg(&tmp)
        .status()
        .is_ok_and(|s| s.success());
    finish(ok, &tmp, out)
}

/// Réunit des extraits de même source en un seul fichier, sans réencodage.
pub fn concat(parts: &[PathBuf], out: &Path) -> bool {
    let Some(ffmpeg) = tool("ffmpeg") else {
        return false;
    };
    if parts.is_empty() {
        return false;
    }
    let list = out.with_extension("liste.txt");
    let lines: String = parts
        .iter()
        .map(|p| {
            // Syntaxe du démultiplexeur concat : apostrophes échappées.
            let path = p
                .to_string_lossy()
                .replace('\\', "/")
                .replace('\'', r"'\''");
            format!("file '{path}'\n")
        })
        .collect();
    if std::fs::write(&list, lines).is_err() {
        return false;
    }
    let tmp = temporary(out);
    let ok = command(&ffmpeg)
        .args(["-v", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-map", "0", "-c", "copy"])
        .arg(&tmp)
        .status()
        .is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&list);
    finish(ok, &tmp, out)
}

/// Fichier temporaire à côté de `out`, de même extension (ffmpeg choisit le
/// format d'après elle).
fn temporary(out: &Path) -> PathBuf {
    let ext = out
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mp4".into());
    out.with_extension(format!("tmp.{ext}"))
}

fn finish(ok: bool, tmp: &Path, out: &Path) -> bool {
    if ok && std::fs::rename(tmp, out).is_ok() {
        return true;
    }
    let _ = std::fs::remove_file(tmp);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/video")
            .join(name)
    }

    #[test]
    fn informations_lues_par_ffprobe() {
        if !available() {
            eprintln!("FFmpeg absent : test sauté (node scripts/fetch-ffmpeg.mjs)");
            return;
        }
        let h264 = probe(&fixture("C0001.MP4")).unwrap();
        assert_eq!(
            (h264.width, h264.height, h264.codec.as_str()),
            (1280, 720, "h264")
        );
        assert_eq!(
            (h264.duration_ms, h264.fps, h264.has_audio),
            (2000, 25.0, true)
        );
        assert!(h264.captured_at.unwrap().starts_with("2026-06-13T"));
        let hevc = probe(&fixture("clip_hevc.mov")).unwrap();
        assert_eq!(
            (hevc.codec.as_str(), hevc.fps, hevc.has_audio),
            ("hevc", 50.0, false)
        );
        assert!(probe(&fixture("C0001M01.XML")).is_none());
    }

    #[test]
    fn coupe_sans_reencodage_sur_les_images_cles() {
        if !available() {
            return;
        }
        let source = fixture("rushes_gop12.mp4");
        let before = std::fs::read(&source).unwrap();
        let keys = keyframes(&source).unwrap();
        assert_eq!(keys, (0..9).map(|i| i * 480).collect::<Vec<u64>>());
        assert_eq!(snap(&keys, 1000), 960);
        assert_eq!(snap(&keys, 480), 480);
        assert_eq!(snap(&[], 1000), 0);

        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp4");
        let b = dir.path().join("b.mp4");
        assert!(cut(&source, 960, 2400, &a));
        assert!(cut(&source, 2880, 3840, &b));
        let (ia, ib) = (probe(&a).unwrap(), probe(&b).unwrap());
        // Sans réencodage, la fin peut déborder de quelques images (et le son
        // d'une trame) : jamais plus court, au plus un quart de seconde de plus.
        let close = |got: i64, want: i64| got + 40 >= want && got <= want + 250;
        assert!(close(ia.duration_ms, 1440), "{}", ia.duration_ms);
        assert!(close(ib.duration_ms, 960), "{}", ib.duration_ms);
        assert!(ia.has_audio && ia.codec == "h264");
        // L'extrait s'ouvre sur une image clé (décalée d'une trame de son au
        // plus, le son commençant un peu avant).
        assert!(keyframes(&a).unwrap().first().is_some_and(|&k| k <= 120));

        let both = dir.path().join("ensemble.mp4");
        assert!(concat(&[a.clone(), b.clone()], &both));
        let joined = probe(&both).unwrap().duration_ms;
        assert!(
            (joined - ia.duration_ms - ib.duration_ms).abs() <= 60,
            "{joined}"
        );
        assert!(!dir.path().join("ensemble.liste.txt").exists());
        // Rien d'écrit à côté de l'original, original intact.
        assert_eq!(std::fs::read(&source).unwrap(), before);
        assert!(!cut(&source, 2000, 1000, &dir.path().join("vide.mp4")));
    }

    #[test]
    fn miniature_et_version_de_lecture() {
        if !available() {
            return;
        }
        let thumb = thumbnail(&fixture("clip_hevc.mov"), 256).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 144));
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("proxy.mp4");
        assert!(proxy(&fixture("clip_hevc.mov"), &out));
        assert_eq!(probe(&out).unwrap().codec, "h264");
        assert!(plays_natively("h264"));
        assert!(!plays_natively("prores"));
    }
}
