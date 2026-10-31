//! Séance de prise de vue reliée (décision 0016) : relevé du boîtier toutes
//! les demi-secondes, et copie de chaque nouvelle photo dès qu'elle paraît
//! dans un dossier de la réception propre à la connexion. L'import (copie
//! vérifiée dans la bibliothèque, album de la séance) se fait ensuite par le
//! chemin des dossiers surveillés.
//!
//! Le boîtier ne liste que les photos prises depuis le branchement et les
//! renumérote à chaque fois (`DSCF0001`…) : une photo reçue est enregistrée
//! par son nom, sa date et sa taille, pour ne pas être relue si Afterdusk
//! redémarre câble branché.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Serialize;

use super::TetherError;
use super::ptp::{Camera, FUJI_REMAINING_SHOTS, ObjectInfo, Transport};
use crate::db::now_utc;

/// Essais de lecture d'une photo avant de la laisser sur la carte.
const ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Update {
    /// Photo copiée en entier dans la réception.
    Received { path: PathBuf, size: u64 },
    /// Photo illisible après plusieurs essais : elle reste sur la carte.
    Failed { name: String, error: String },
    /// Photos que la carte peut encore recevoir (estimation du boîtier).
    Remaining { shots: u32 },
}

pub struct Session<T> {
    camera: Camera<T>,
    model: String,
    serial: String,
    reports_changes: bool,
    /// Dossier de la connexion, créé à la première photo.
    dir: PathBuf,
    /// Objets traités : reçus, déjà reçus avant, dossiers, ou abandonnés.
    done: HashSet<u32>,
    failures: HashMap<u32, u8>,
    /// Objets de la dernière liste qui ne sont pas encore reçus.
    waiting: usize,
    started: bool,
}

/// Nom de dossier sans caractère refusé par Windows ou macOS.
fn folder_name(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "boitier".to_owned()
    } else {
        clean
    }
}

impl<T: Transport> Session<T> {
    /// `reception` : dossier de réception des boîtiers ; la connexion y
    /// prend `<modèle>-<numéro>/<date-heure>`.
    pub fn open(camera: Camera<T>, reception: &Path) -> Result<Self, TetherError> {
        let info = camera.info()?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let dir = reception
            .join(folder_name(&format!("{}-{}", info.model, info.serial)))
            .join(stamp);
        Ok(Self {
            camera,
            reports_changes: info.reports_changes(),
            model: info.model,
            serial: info.serial,
            dir,
            done: HashSet::new(),
            failures: HashMap::new(),
            waiting: 0,
            started: false,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn camera(&self) -> &Camera<T> {
        &self.camera
    }

    /// Photos listées par le boîtier et pas encore reçues : au débranchement,
    /// elles restent sur la carte.
    pub fn waiting(&self) -> usize {
        self.waiting
    }

    fn already_received(&self, conn: &Connection, o: &ObjectInfo) -> Result<bool, TetherError> {
        let Some(size) = o.size else {
            return Ok(false);
        };
        Ok(conn
            .query_row(
                "SELECT 1 FROM tether_received
                  WHERE serial = ?1 AND name = ?2 AND captured = ?3 AND size = ?4",
                params![self.serial, o.name, o.captured, size as i64],
                |_| Ok(()),
            )
            .is_ok())
    }

    fn record(&self, conn: &Connection, o: &ObjectInfo, size: u64) -> Result<(), TetherError> {
        conn.execute(
            "INSERT OR IGNORE INTO tether_received (serial, name, captured, size, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![self.serial, o.name, o.captured, size as i64, now_utc()],
        )?;
        Ok(())
    }

    /// Un relevé : place restante si elle a changé, puis chaque nouvelle
    /// photo lue dans la réception. Une erreur `Disconnected` met fin à la
    /// connexion.
    pub fn poll(&mut self, conn: &Connection) -> Result<Vec<Update>, TetherError> {
        let mut updates = Vec::new();
        if !self.started {
            self.started = true;
            // Changements d'avant la séance : déjà comptés dans la place lue.
            if self.reports_changes {
                self.camera.changes()?;
            }
            if let Ok(shots) = self.camera.remaining_shots() {
                updates.push(Update::Remaining { shots });
            }
        } else if self.reports_changes
            && let Some((_, shots)) = self
                .camera
                .changes()?
                .into_iter()
                .rfind(|(code, _)| u32::from(*code) == FUJI_REMAINING_SHOTS)
        {
            updates.push(Update::Remaining { shots });
        }

        let handles = self.camera.handles()?;
        let fresh: Vec<u32> = handles
            .iter()
            .filter(|h| !self.done.contains(h))
            .copied()
            .collect();
        self.waiting = fresh.len();
        for handle in fresh {
            let info = self.camera.object(handle)?;
            if info.format == ObjectInfo::FOLDER || self.already_received(conn, &info)? {
                self.done.insert(handle);
                self.waiting -= 1;
                continue;
            }
            std::fs::create_dir_all(&self.dir)?;
            let target = self.dir.join(&info.name);
            match self.camera.download(handle, &info, &target) {
                Ok(size) => {
                    self.record(conn, &info, size)?;
                    self.done.insert(handle);
                    self.waiting -= 1;
                    updates.push(Update::Received { path: target, size });
                }
                Err(TetherError::Disconnected) => return Err(TetherError::Disconnected),
                Err(e) => {
                    let tries = self.failures.entry(handle).or_insert(0);
                    *tries += 1;
                    if *tries >= ATTEMPTS {
                        self.done.insert(handle);
                        self.waiting -= 1;
                        updates.push(Update::Failed {
                            name: info.name.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }
        Ok(updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::tether::fake::FakeCamera;

    fn received(updates: &[Update]) -> Vec<String> {
        updates
            .iter()
            .filter_map(|u| match u {
                Update::Received { path, .. } => {
                    Some(path.file_name()?.to_string_lossy().into_owned())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chaque_photo_est_copiee_des_qu_elle_parait() {
        let conn = open_in_memory().unwrap();
        let reception = tempfile::tempdir().unwrap();
        let mut session =
            Session::open(Camera::new(FakeCamera::new(&[], 30)), reception.path()).unwrap();
        assert_eq!(session.model(), "X-H2");
        let first = session.poll(&conn).unwrap();
        assert_eq!(first, vec![Update::Remaining { shots: 30 }]);
        // Aucune photo : pas de dossier vide dans la réception.
        assert_eq!(std::fs::read_dir(reception.path()).unwrap().count(), 0);

        let fake = session.camera().transport();
        fake.shoot("DSCF0001.raf", vec![1; 4000]);
        fake.shoot("DSCF0002.jpg", vec![2; 1000]);
        let updates = session.poll(&conn).unwrap();
        assert_eq!(updates[0], Update::Remaining { shots: 28 });
        assert_eq!(received(&updates), vec!["DSCF0001.raf", "DSCF0002.jpg"]);
        let Update::Received { path, size } = &updates[1] else {
            panic!("photo attendue");
        };
        assert_eq!(*size, 4000);
        assert_eq!(std::fs::read(path).unwrap(), vec![1; 4000]);
        assert!(path.starts_with(reception.path().join("X-H2-FAKE0001")));
        assert_eq!(session.waiting(), 0);
        // Rien de neuf : rien à faire.
        assert!(session.poll(&conn).unwrap().is_empty());
    }

    #[test]
    fn redemarrage_cable_branche_sans_relire() {
        let conn = open_in_memory().unwrap();
        let reception = tempfile::tempdir().unwrap();
        let fake = FakeCamera::new(&[], 30);
        fake.shoot("DSCF0001.raf", vec![1; 4000]);
        let mut session = Session::open(Camera::new(fake), reception.path()).unwrap();
        assert_eq!(
            received(&session.poll(&conn).unwrap()),
            vec!["DSCF0001.raf"]
        );

        // Afterdusk redémarre : le boîtier liste encore la photo.
        let fake = FakeCamera::new(&[], 29);
        fake.shoot("DSCF0001.raf", vec![1; 4000]);
        let mut again = Session::open(Camera::new(fake), reception.path()).unwrap();
        assert!(received(&again.poll(&conn).unwrap()).is_empty());
        let sent = again.camera().transport().sent();
        assert!(!sent.contains(&crate::tether::ptp::GET_OBJECT));

        // Même nom à une autre connexion, mais autre photo : reçue.
        again
            .camera()
            .transport()
            .shoot("DSCF0002.raf", vec![3; 4100]);
        assert_eq!(received(&again.poll(&conn).unwrap()), vec!["DSCF0002.raf"]);
    }

    #[test]
    fn debranchement_en_cours_de_lecture() {
        let conn = open_in_memory().unwrap();
        let reception = tempfile::tempdir().unwrap();
        let mut session =
            Session::open(Camera::new(FakeCamera::new(&[], 30)), reception.path()).unwrap();
        session.poll(&conn).unwrap();
        let fake = session.camera().transport();
        fake.shoot("DSCF0001.raf", vec![1; 4000]);
        fake.shoot("DSCF0002.jpg", vec![2; 1000]);
        fake.cut_next_transfer();
        assert!(matches!(
            session.poll(&conn),
            Err(TetherError::Disconnected)
        ));
        // Les deux photos restent sur la carte, aucune n'est à moitié reçue.
        assert_eq!(session.waiting(), 2);
        let files: Vec<_> = walkdir::WalkDir::new(reception.path())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .collect();
        assert!(files.is_empty());
        assert_eq!(session.camera().transport().card().len(), 2);
    }

    #[test]
    fn photo_illisible_abandonnee_apres_trois_essais() {
        struct Broken(FakeCamera);
        impl Transport for Broken {
            fn read(
                &self,
                op: crate::tether::ptp::Operation,
                params: &[u32],
                sink: &mut dyn std::io::Write,
            ) -> Result<u16, TetherError> {
                if op.code() == crate::tether::ptp::GET_OBJECT {
                    return Ok(0x2002); // General_Error
                }
                self.0.read(op, params, sink)
            }
        }
        let conn = open_in_memory().unwrap();
        let reception = tempfile::tempdir().unwrap();
        let fake = FakeCamera::new(&[], 30);
        fake.shoot("DSCF0001.raf", vec![1; 4000]);
        let mut session = Session::open(Camera::new(Broken(fake)), reception.path()).unwrap();
        assert!(received(&session.poll(&conn).unwrap()).is_empty());
        assert!(session.poll(&conn).unwrap().is_empty());
        let third = session.poll(&conn).unwrap();
        assert!(matches!(
            third.as_slice(),
            [Update::Failed { name, .. }] if name == "DSCF0001.raf"
        ));
        assert!(session.poll(&conn).unwrap().is_empty());
        assert_eq!(session.waiting(), 0);
    }
}
