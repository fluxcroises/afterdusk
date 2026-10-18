use std::path::PathBuf;

use sha2::{Digest, Sha256};

pub fn test_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test-assets")
}

/// Échantillon de test-assets/, ou panique avec la commande à lancer.
pub fn sample(name: &str) -> PathBuf {
    let path = test_assets().join(name);
    assert!(
        path.exists(),
        "{} absent : lancez `node scripts/fetch-test-assets.mjs`",
        path.display()
    );
    path
}

pub fn sha256(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("lecture de l'échantillon");
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[allow(dead_code)]
pub fn modified(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .expect("date de modification")
}
