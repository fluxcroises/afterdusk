//! Réception FTP sur le réseau local (cahier des charges, 4.1 bis ; décision
//! 0009). Désactivée par défaut. Un serveur FTP (libunftp) reçoit les
//! envois des boîtiers dans un dossier de réception, seul dossier auquel il
//! a accès ; chaque fichier reçu en entier part à l'import par le chemin du
//! dossier surveillé. FTP et FTPS sont acceptés, FTPS conseillé ; seules les
//! adresses du réseau local peuvent se connecter.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libunftp::notification::{DataEvent, DataListener, EventMeta};
use libunftp::options::{FtpsRequired, Shutdown};
use libunftp::{Server, ServerBuilder};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unftp_core::auth::{AuthenticationError, Authenticator, Credentials, DefaultUser, Principal};
use unftp_sbe_fs::Filesystem;

const KEY: &str = "ftp.reception";
/// Version des réglages enregistrés ; toute évolution prévoit une migration
/// dans `decode`.
pub const SETTINGS_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 2121;
/// Ports des transferts en mode passif.
pub const PASSIVE_PORTS: std::ops::RangeInclusive<u16> = 50000..=50100;
/// Sans 0, O, 1, I ni L : un mot de passe se recopie sans erreur sur un boîtier.
const PASSWORD_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

#[derive(Debug, thiserror::Error)]
pub enum FtpError {
    #[error("réglages FTP : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("dossier de réception : {0}")]
    Io(#[from] std::io::Error),
    #[error("certificat : {0}")]
    Certificate(String),
    #[error("serveur FTP : {0}")]
    Server(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FtpSettings {
    pub enabled: bool,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// FTP non chiffré accepté aussi (mot de passe et photos en clair).
    pub allow_plain: bool,
    pub library_id: Option<i64>,
    pub album_name: String,
}

impl Default for FtpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            user: "afterdusk".to_owned(),
            password: String::new(),
            allow_plain: false,
            library_id: None,
            album_name: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    settings: FtpSettings,
}

fn decode(json: &str) -> FtpSettings {
    match serde_json::from_str::<Stored>(json) {
        // Version 1 : format actuel.
        Ok(stored) if stored.version <= SETTINGS_VERSION => stored.settings,
        _ => FtpSettings::default(),
    }
}

/// Mot de passe de 12 caractères, propre à Afterdusk.
pub fn new_password() -> String {
    let seed = format!(
        "{:?}|{}|{:?}",
        std::time::SystemTime::now(),
        std::process::id(),
        std::thread::current().id()
    );
    blake3::hash(seed.as_bytes()).as_bytes()[..12]
        .iter()
        .map(|b| PASSWORD_ALPHABET[usize::from(*b) % PASSWORD_ALPHABET.len()] as char)
        .collect()
}

/// Réglages enregistrés ; un mot de passe est créé la première fois.
pub fn get(conn: &Connection) -> Result<FtpSettings, FtpError> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    let mut settings = value.as_deref().map(decode).unwrap_or_default();
    if settings.password.is_empty() {
        settings.password = new_password();
        set(conn, &settings)?;
    }
    Ok(settings)
}

pub fn set(conn: &Connection, settings: &FtpSettings) -> Result<(), FtpError> {
    let mut s = settings.clone();
    s.user = s.user.trim().to_owned();
    if s.user.is_empty() {
        s.user = FtpSettings::default().user;
    }
    if s.port < 1024 {
        s.port = DEFAULT_PORT;
    }
    let json = serde_json::to_string(&Stored {
        version: SETTINGS_VERSION,
        settings: s,
    })
    .unwrap_or_else(|_| "{}".to_owned());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [KEY, json.as_str()],
    )?;
    Ok(())
}

/// Dossier de réception, dans les données d'Afterdusk.
pub fn reception_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("reception-ftp")
}

/// Adresse du réseau local, de la machine elle-même ou de lien local.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            // Adresses uniques locales (fc00::/7) et de lien local (fe80::/10).
            v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// Identifiant et mot de passe d'Afterdusk, depuis le réseau local seulement.
#[derive(Debug)]
pub struct LocalAuthenticator {
    user: String,
    password: String,
}

impl LocalAuthenticator {
    pub fn new(user: &str, password: &str) -> Self {
        Self {
            user: user.to_owned(),
            password: password.to_owned(),
        }
    }
}

#[async_trait::async_trait]
impl Authenticator for LocalAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<Principal, AuthenticationError> {
        if !is_local(creds.source_ip) {
            return Err(AuthenticationError::IpDisallowed);
        }
        if username != self.user {
            return Err(AuthenticationError::BadUser);
        }
        if creds.password.as_deref() != Some(self.password.as_str()) {
            return Err(AuthenticationError::BadPassword);
        }
        Ok(Principal {
            username: username.to_owned(),
        })
    }
}

/// Certificat auto-signé du serveur FTPS : PEM pour le serveur, DER (`.cer`)
/// pour les boîtiers qui demandent de l'importer, empreinte SHA-256.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub der: PathBuf,
    /// Empreinte à comparer sur le boîtier, en paires hexadécimales.
    pub fingerprint: String,
}

/// Crée le certificat la première fois, puis le relit.
pub fn certificate(data_dir: &Path) -> Result<Certificate, FtpError> {
    let dir = data_dir.join("ftp");
    let (cert, key, der) = (
        dir.join("certificat.pem"),
        dir.join("cle.pem"),
        dir.join("afterdusk.cer"),
    );
    if !(cert.is_file() && key.is_file() && der.is_file()) {
        std::fs::create_dir_all(&dir)?;
        let generated = rcgen::generate_simple_self_signed(vec![
            "afterdusk.local".to_owned(),
            "localhost".to_owned(),
        ])
        .map_err(|e| FtpError::Certificate(e.to_string()))?;
        std::fs::write(&key, generated.signing_key.serialize_pem())?;
        std::fs::write(&cert, generated.cert.pem())?;
        std::fs::write(&der, generated.cert.der())?;
    }
    let bytes = std::fs::read(&der)?;
    let fingerprint = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    Ok(Certificate {
        cert,
        key,
        der,
        fingerprint,
    })
}

/// Prévient d'un fichier reçu en entier (commande STOR terminée).
struct Received {
    root: PathBuf,
    on_file: Arc<dyn Fn(PathBuf) + Send + Sync>,
}

impl std::fmt::Debug for Received {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Received")
            .field("root", &self.root)
            .finish()
    }
}

#[async_trait::async_trait]
impl DataListener for Received {
    async fn receive_data_event(&self, e: DataEvent, _m: EventMeta) {
        if let DataEvent::Put { path, .. } = e {
            (self.on_file)(self.root.join(path.trim_start_matches(['/', '\\'])));
        }
    }
}

/// Serveur prêt à écouter : stockage limité au dossier de réception,
/// connexion réservée au réseau local, FTPS exigé sauf si le FTP simple est
/// accepté. `on_file` reçoit chaque fichier complet ; `shutdown` l'arrête.
pub fn server(
    settings: &FtpSettings,
    reception: &Path,
    cert: &Certificate,
    on_file: Arc<dyn Fn(PathBuf) + Send + Sync>,
    shutdown: impl std::future::Future<Output = ()> + Send + Sync + 'static,
) -> Result<Server<Filesystem, DefaultUser>, FtpError> {
    std::fs::create_dir_all(reception)?;
    let root = reception.to_path_buf();
    let auth = Arc::new(LocalAuthenticator::new(&settings.user, &settings.password));
    let storage = root.clone();
    let required = if settings.allow_plain {
        FtpsRequired::None
    } else {
        FtpsRequired::All
    };
    ServerBuilder::with_authenticator(
        Box::new(move || Filesystem::new(storage.clone()).expect("dossier de réception")),
        auth,
    )
    .greeting("Afterdusk")
    .passive_ports(PASSIVE_PORTS)
    .ftps(cert.cert.clone(), cert.key.clone())
    .ftps_required(required, required)
    .notify_data(Received { root, on_file })
    .shutdown_indicator(async move {
        shutdown.await;
        Shutdown::new().grace_period(std::time::Duration::from_secs(2))
    })
    .build()
    .map_err(|e| FtpError::Server(e.to_string()))
}
