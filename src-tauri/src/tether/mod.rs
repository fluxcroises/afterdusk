//! Connexion au boîtier (V0.8, décision 0016) : une photo déclenchée sur le
//! boîtier relié en USB arrive d'elle-même dans Afterdusk. Le boîtier parle
//! PTP ; sous Windows, à travers Windows Portable Devices, sans changer de
//! pilote.
//!
//! Rien n'est jamais écrit ni supprimé sur le boîtier : seules les
//! opérations de lecture de `ptp::ALLOWED` peuvent lui être envoyées, et
//! `ptp::Operation` ne se construit qu'à partir de cette liste.

pub mod fake;
pub mod ptp;
pub mod session;
#[cfg(windows)]
pub mod wpd;

#[derive(Debug, thiserror::Error)]
pub enum TetherError {
    #[error("opération PTP {0:04X} refusée : elle n'est pas dans la liste des lectures")]
    Refused(u16),
    #[error("le boîtier a répondu {code:04X} à l'opération {op:04X}")]
    Response { op: u16, code: u16 },
    #[error("boîtier débranché")]
    Disconnected,
    #[error("réponse du boîtier illisible : {0}")]
    Malformed(&'static str),
    #[error("photo reçue incomplète : {got} octets sur {expected}")]
    Incomplete { expected: u64, got: u64 },
    #[error("Windows Portable Devices : {0}")]
    Device(String),
    #[error("réception : {0}")]
    Io(#[from] std::io::Error),
    #[error("photos reçues : {0}")]
    Sqlite(#[from] rusqlite::Error),
}
