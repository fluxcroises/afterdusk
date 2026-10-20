//! Notes, drapeaux et couleurs depuis l'interface, et fil d'écriture des XMP.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use tauri::State;

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::db;
use crate::library::LibraryError;
use crate::library::copies;
use crate::library::marks::{self, Mark, MarkError};
use crate::metadata::xmp::{ColorLabel, Flag};

/// Photos écrites par passage ; le reste suit au passage suivant.
const BATCH: usize = 200;
/// Regroupe les frappes rapprochées (tri au clavier) en une seule écriture.
const DEBOUNCE: Duration = Duration::from_millis(400);
/// Nouvelle tentative régulière, pour une bibliothèque rebranchée.
const RETRY: Duration = Duration::from_secs(60);

impl From<MarkError> for CommandError {
    fn from(e: MarkError) -> Self {
        let code = match &e {
            MarkError::InvalidRating(_) => "invalidMark",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

/// Réveille le fil d'écriture des XMP.
pub struct XmpWriter {
    sender: Mutex<Sender<()>>,
}

impl XmpWriter {
    pub fn start(db_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let Ok(conn) = db::connect(&db_path) else {
                return;
            };
            loop {
                match receiver.recv_timeout(RETRY) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                std::thread::sleep(DEBOUNCE);
                while receiver.try_recv().is_ok() {}
                loop {
                    match marks::flush_pending_xmp(&conn, BATCH) {
                        Ok(report) if report.written == BATCH => continue,
                        Ok(_) => break,
                        Err(error) => {
                            eprintln!("écriture des XMP : {error}");
                            break;
                        }
                    }
                }
            }
        });
        let writer = Self {
            sender: Mutex::new(sender),
        };
        // Reliquat d'une session précédente (application fermée avant l'écriture).
        writer.wake();
        writer
    }

    pub fn wake(&self) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(());
        }
    }
}

async fn apply(
    state: &AppState,
    writer: &XmpWriter,
    ids: Vec<i64>,
    mark: Mark,
) -> CommandResult<usize> {
    let mut conn = state.connect()?;
    let changed = blocking(move || {
        // Copies virtuelles : les marques sont celles de leur photo.
        let ids = copies::media_ids(&conn, &ids).map_err(LibraryError::from)?;
        Ok(marks::apply_mark(&mut conn, &ids, mark)?)
    })
    .await?;
    if changed > 0 {
        writer.wake();
    }
    Ok(changed)
}

#[tauri::command]
pub async fn media_set_rating(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
    rating: u8,
) -> CommandResult<usize> {
    apply(&state, &writer, ids, Mark::Rating(rating)).await
}

#[tauri::command]
pub async fn media_set_flag(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
    flag: i64,
) -> CommandResult<usize> {
    if !(-1..=1).contains(&flag) {
        return Err(CommandError::new("invalidMark", format!("drapeau {flag}")));
    }
    apply(&state, &writer, ids, Mark::Flag(Flag::from_db(flag))).await
}

#[tauri::command]
pub async fn media_set_color(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
    color: Option<String>,
) -> CommandResult<usize> {
    let label = match color.as_deref() {
        None => None,
        Some(c) => Some(
            ColorLabel::from_db(c)
                .ok_or_else(|| CommandError::new("invalidMark", format!("couleur {c}")))?,
        ),
    };
    apply(&state, &writer, ids, Mark::Color(label)).await
}
