//! Réception FTP de bout en bout (décision 0009) avec un vrai client FTP :
//! envoi en FTP simple puis en FTPS, mauvais mot de passe refusé, FTP
//! simple refusé quand FTPS est exigé, rien d'écrit hors du dossier de
//! réception, adresse publique refusée, photo reçue importée dans son album.

mod common;

use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use afterdusk_lib::ftp::{self, FtpSettings, LocalAuthenticator};
use afterdusk_lib::import::auto::AutoSettings;
use afterdusk_lib::import::watch;
use afterdusk_lib::library::media::{MediaQuery, list_albums, list_media};
use afterdusk_lib::{db, library};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use suppaftp::{FtpStream, RustlsConnector, RustlsFtpStream};
use unftp_core::auth::{AuthenticationError, Authenticator, ChannelEncryptionState, Credentials};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Fait confiance au seul certificat dont l'empreinte est connue, comme un
/// boîtier à qui l'on a confirmé l'empreinte.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let seen = Sha256::digest(end_entity.as_ref())
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":");
        if seen == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("empreinte inattendue".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Serveur de réception lancé sur un port libre ; les fichiers reçus en
/// entier sont notés.
fn start(
    runtime: &tokio::runtime::Runtime,
    settings: &FtpSettings,
    data: &std::path::Path,
) -> (
    u16,
    Arc<Mutex<Vec<PathBuf>>>,
    tokio::sync::oneshot::Sender<()>,
) {
    let port = free_port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let noted = received.clone();
    let cert = ftp::certificate(data).unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = ftp::server(
        settings,
        &ftp::reception_dir(data),
        &cert,
        Arc::new(move |p| noted.lock().unwrap().push(p)),
        async move {
            let _ = stopped.await;
        },
    )
    .unwrap();
    runtime.spawn(server.listen(format!("127.0.0.1:{port}")));
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "serveur FTP absent");
        std::thread::sleep(Duration::from_millis(50));
    }
    (port, received, stop)
}

fn wait_for(received: &Arc<Mutex<Vec<PathBuf>>>, count: usize) -> Vec<PathBuf> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let list = received.lock().unwrap().clone();
        if list.len() >= count {
            return list;
        }
        assert!(Instant::now() < deadline, "envoi jamais signalé");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn reception_ftp_et_ftps() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("donnees");
    std::fs::create_dir_all(&data).unwrap();
    let root = dir.path().join("Bibliotheque");
    std::fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&data.join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let reception = ftp::reception_dir(&data);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sample = common::sample(common::RAW_SAMPLES[0]);
    let name = sample.file_name().unwrap().to_string_lossy().into_owned();

    // FTP simple accepté.
    let plain = FtpSettings {
        enabled: true,
        password: "motdepasse23".into(),
        allow_plain: true,
        library_id: Some(lib.id),
        album_name: "Réception FTP".into(),
        ..Default::default()
    };
    ftp::set(&conn, &plain).unwrap();
    let folder_id = watch::ensure_reception(
        &conn,
        &reception,
        lib.id,
        "Réception FTP",
        &AutoSettings::default(),
        true,
    )
    .unwrap();
    let (port, received, stop) = start(&runtime, &plain, &data);
    let mut client = FtpStream::connect(("127.0.0.1", port)).unwrap();
    assert!(client.login("afterdusk", "mauvais").is_err());
    client.login("afterdusk", "motdepasse23").unwrap();
    client
        .transfer_type(suppaftp::types::FileType::Binary)
        .unwrap();
    // Le dossier racine est la réception : on ne peut pas en sortir.
    let escaped = client.put_file("../evasion.txt", &mut &b"hors de la reception"[..]);
    assert!(escaped.is_err() || !data.join("evasion.txt").exists());
    assert!(!data.join("evasion.txt").exists());
    let start_send = Instant::now();
    client.mkdir("DCIM").unwrap();
    client
        .put_file(
            format!("DCIM/{name}"),
            &mut std::fs::File::open(&sample).unwrap(),
        )
        .unwrap();
    client.quit().unwrap();
    let got = wait_for(&received, 1);
    let arrived = got.iter().find(|p| p.ends_with(&name)).unwrap().clone();
    assert!(arrived.starts_with(&reception));
    assert_eq!(common::sha256(&arrived), common::sha256(&sample));

    // Le fichier reçu part à l'import dans l'album de la réception.
    let folder = watch::list(&conn)
        .unwrap()
        .into_iter()
        .find(|f| f.id == folder_id)
        .unwrap();
    let imported = watch::import_files(
        &mut conn,
        &folder,
        vec![arrived.clone()],
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
    .unwrap();
    let elapsed = start_send.elapsed();
    assert_eq!(imported.media_ids.len(), 1);
    assert!(
        elapsed < Duration::from_secs(5),
        "photo importée après {elapsed:?}"
    );
    println!("réception FTP : photo envoyée puis importée en {elapsed:?}");
    assert_eq!(list_albums(&conn, lib.id).unwrap()[0].name, "Réception FTP");
    assert_eq!(
        list_media(&conn, lib.id, &MediaQuery::default())
            .unwrap()
            .len(),
        1
    );
    let _ = stop.send(());

    // FTPS exigé : le FTP simple est refusé, le FTPS passe.
    let secure = FtpSettings {
        allow_plain: false,
        ..plain.clone()
    };
    let (port, received, stop) = start(&runtime, &secure, &data);
    let mut refused = FtpStream::connect(("127.0.0.1", port)).unwrap();
    assert!(refused.login("afterdusk", "motdepasse23").is_err());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            fingerprint: ftp::certificate(&data).unwrap().fingerprint,
            provider,
        }))
        .with_no_client_auth();
    let mut tls = RustlsFtpStream::connect(("127.0.0.1", port))
        .unwrap()
        .into_secure(RustlsConnector::from(Arc::new(config)), "localhost")
        .unwrap();
    tls.login("afterdusk", "motdepasse23").unwrap();
    tls.transfer_type(suppaftp::types::FileType::Binary)
        .unwrap();
    tls.put_file("seconde.RAF", &mut &b"photo chiffree"[..])
        .unwrap();
    tls.quit().unwrap();
    let got = wait_for(&received, 1);
    assert_eq!(got[0], reception.join("seconde.RAF"));
    let _ = stop.send(());
}

#[test]
fn reseau_local_seulement() {
    for (ip, local) in [
        ("192.168.1.20", true),
        ("10.0.0.5", true),
        ("172.20.1.1", true),
        ("169.254.10.2", true),
        ("127.0.0.1", true),
        ("fe80::1", true),
        ("fd12:3456::1", true),
        ("::ffff:192.168.1.20", true),
        ("8.8.8.8", false),
        ("172.32.0.1", false),
        ("2001:4860:4860::8888", false),
    ] {
        assert_eq!(ftp::is_local(ip.parse::<IpAddr>().unwrap()), local, "{ip}");
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let auth = LocalAuthenticator::new("afterdusk", "motdepasse23");
    let creds = |ip: &str| Credentials {
        password: Some("motdepasse23".into()),
        certificate_chain: None,
        source_ip: ip.parse().unwrap(),
        command_channel_security: ChannelEncryptionState::Plaintext,
    };
    assert!(matches!(
        runtime.block_on(auth.authenticate("afterdusk", &creds("8.8.8.8"))),
        Err(AuthenticationError::IpDisallowed)
    ));
    assert!(
        runtime
            .block_on(auth.authenticate("afterdusk", &creds("192.168.1.20")))
            .is_ok()
    );
}

#[test]
fn reglages_et_certificat() {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("a.db")).unwrap();
    // Désactivée par défaut, avec un mot de passe propre à Afterdusk.
    let s = ftp::get(&conn).unwrap();
    assert!(!s.enabled && !s.allow_plain);
    assert_eq!(
        (s.port, s.user.as_str(), s.password.len()),
        (2121, "afterdusk", 12)
    );
    assert_eq!(ftp::get(&conn).unwrap().password, s.password);
    let cert = ftp::certificate(dir.path()).unwrap();
    assert_eq!(cert.fingerprint.len(), 32 * 3 - 1);
    // Relu, pas recréé.
    assert_eq!(
        ftp::certificate(dir.path()).unwrap().fingerprint,
        cert.fingerprint
    );
}
