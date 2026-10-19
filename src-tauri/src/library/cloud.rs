//! Dossiers synchronisés par les services de cloud (OneDrive, Dropbox, Google
//! Drive, iCloud Drive…), proposés comme sauvegarde. Afterdusk n'y fait que
//! copier des fichiers : l'envoi en ligne est l'affaire du logiciel du service,
//! sans aucun appel réseau d'Afterdusk (règle absolue du projet).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Un dossier synchronisé trouvé sur l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFolder {
    /// Service : `onedrive`, `dropbox`, `googledrive`, `icloud`, `box`,
    /// `synology`, `nextcloud`, `pcloud`.
    pub service: &'static str,
    /// Nom affiché : « OneDrive », « OneDrive - Entreprise », « Dropbox »…
    pub label: String,
    pub path: PathBuf,
}

/// Où chercher : variables d'environnement et dossiers personnels.
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub home: Option<PathBuf>,
    /// `(nom, valeur)` des variables utiles (OneDrive, APPDATA, LOCALAPPDATA).
    pub env: Vec<(String, String)>,
}

impl Places {
    pub fn current() -> Self {
        let names = [
            "OneDrive",
            "OneDriveConsumer",
            "OneDriveCommercial",
            "APPDATA",
            "LOCALAPPDATA",
        ];
        Self {
            home: std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(PathBuf::from),
            env: names
                .iter()
                .filter_map(|n| std::env::var(n).ok().map(|v| ((*n).to_owned(), v)))
                .collect(),
        }
    }

    fn var(&self, name: &str) -> Option<PathBuf> {
        self.env
            .iter()
            .find(|(n, v)| n == name && !v.is_empty())
            .map(|(_, v)| PathBuf::from(v))
    }
}

/// Dossiers Dropbox déclarés par l'application de bureau (`info.json`).
fn dropbox_folders(places: &Places) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for base in [places.var("APPDATA"), places.var("LOCALAPPDATA")]
        .into_iter()
        .flatten()
        .chain(places.home.as_ref().map(|h| h.join(".dropbox")))
    {
        let file = if base.ends_with(".dropbox") {
            base.join("info.json")
        } else {
            base.join("Dropbox").join("info.json")
        };
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        for (kind, label) in [("personal", "Dropbox"), ("business", "Dropbox Business")] {
            if let Some(path) = json[kind]["path"].as_str() {
                found.push((label.to_owned(), PathBuf::from(path)));
            }
        }
    }
    found
}

/// Nom affiché d'un dossier de `~/Library/CloudStorage` (macOS) :
/// `OneDrive-Personnel` → service `onedrive`, « OneDrive - Personnel ».
fn cloud_storage_entry(name: &str) -> Option<(&'static str, String)> {
    let (prefix, rest) = name.split_once('-').unwrap_or((name, ""));
    let service = match prefix {
        "OneDrive" => "onedrive",
        "Dropbox" => "dropbox",
        "GoogleDrive" => "googledrive",
        "Box" => "box",
        "pCloud" => "pcloud",
        "Nextcloud" => "nextcloud",
        "SynologyDrive" => "synology",
        _ => return None,
    };
    let base = match service {
        "googledrive" => "Google Drive",
        "synology" => "Synology Drive",
        _ => prefix,
    };
    let label = if rest.is_empty() {
        base.to_owned()
    } else {
        format!("{base} - {rest}")
    };
    Some((service, label))
}

/// Dossiers synchronisés présents, sans doublon.
pub fn detect(places: &Places) -> Vec<CloudFolder> {
    let mut candidates: Vec<(&'static str, String, PathBuf)> = Vec::new();
    for (var, label) in [
        ("OneDriveConsumer", "OneDrive"),
        ("OneDrive", "OneDrive"),
        ("OneDriveCommercial", "OneDrive - Entreprise"),
    ] {
        if let Some(p) = places.var(var) {
            candidates.push(("onedrive", label.to_owned(), p));
        }
    }
    for (label, path) in dropbox_folders(places) {
        candidates.push(("dropbox", label, path));
    }
    if let Some(home) = &places.home {
        // macOS : un dossier par compte dans ~/Library/CloudStorage.
        if let Ok(entries) = std::fs::read_dir(home.join("Library").join("CloudStorage")) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if let Some((service, label)) = cloud_storage_entry(&name) {
                    candidates.push((service, label, e.path()));
                }
            }
        }
        let fixed: [(&'static str, &str, PathBuf); 9] = [
            (
                "icloud",
                "iCloud Drive",
                home.join("Library")
                    .join("Mobile Documents")
                    .join("com~apple~CloudDocs"),
            ),
            ("icloud", "iCloud Drive", home.join("iCloudDrive")),
            ("dropbox", "Dropbox", home.join("Dropbox")),
            ("googledrive", "Google Drive", home.join("Google Drive")),
            ("googledrive", "Google Drive", home.join("My Drive")),
            ("box", "Box", home.join("Box")),
            ("synology", "Synology Drive", home.join("SynologyDrive")),
            ("nextcloud", "Nextcloud", home.join("Nextcloud")),
            ("pcloud", "pCloud", home.join("pCloud Drive")),
        ];
        for (service, label, path) in fixed {
            candidates.push((service, label.to_owned(), path));
        }
    }
    let mut found: Vec<CloudFolder> = Vec::new();
    for (service, label, path) in candidates {
        if !path.is_dir() {
            continue;
        }
        let key = path.canonicalize().unwrap_or_else(|_| path.clone());
        let duplicate = found
            .iter()
            .any(|f| f.path.canonicalize().unwrap_or_else(|_| f.path.clone()) == key);
        if !duplicate {
            found.push(CloudFolder {
                service,
                label,
                path,
            });
        }
    }
    found
}

/// Le dossier est-il dans un dossier synchronisé ? Lequel ?
pub fn containing<'a>(folders: &'a [CloudFolder], path: &Path) -> Option<&'a CloudFolder> {
    let normalize = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    };
    let target = normalize(path);
    folders.iter().find(|f| {
        let base = normalize(&f.path);
        target == base || target.starts_with(&format!("{base}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossiers_trouves_et_sans_doublon() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("lewis");
        let onedrive = home.join("OneDrive");
        let dropbox = home.join("Dropbox Perso");
        std::fs::create_dir_all(&onedrive).unwrap();
        std::fs::create_dir_all(&dropbox).unwrap();
        std::fs::create_dir_all(home.join("SynologyDrive")).unwrap();
        // Dropbox déclare son dossier dans info.json.
        let appdata = dir.path().join("AppData");
        std::fs::create_dir_all(appdata.join("Dropbox")).unwrap();
        std::fs::write(
            appdata.join("Dropbox").join("info.json"),
            serde_json::json!({ "personal": { "path": dropbox } }).to_string(),
        )
        .unwrap();
        let places = Places {
            home: Some(home.clone()),
            env: vec![
                ("OneDrive".into(), onedrive.to_string_lossy().into_owned()),
                (
                    "OneDriveConsumer".into(),
                    onedrive.to_string_lossy().into_owned(),
                ),
                ("APPDATA".into(), appdata.to_string_lossy().into_owned()),
            ],
        };
        let found = detect(&places);
        let labels: Vec<&str> = found.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, ["OneDrive", "Dropbox", "Synology Drive"]);

        let backup = onedrive.join("Afterdusk");
        assert_eq!(
            containing(&found, &backup).map(|f| f.service),
            Some("onedrive")
        );
        assert!(containing(&found, &home.join("OneDriveCopie")).is_none());
    }

    #[test]
    fn comptes_de_cloudstorage_sur_mac() {
        assert_eq!(
            cloud_storage_entry("OneDrive-Personnel"),
            Some(("onedrive", "OneDrive - Personnel".to_owned()))
        );
        assert_eq!(
            cloud_storage_entry("GoogleDrive-lewis@example.org"),
            Some(("googledrive", "Google Drive - lewis@example.org".to_owned()))
        );
        assert_eq!(cloud_storage_entry("Inconnu"), None);
    }
}
