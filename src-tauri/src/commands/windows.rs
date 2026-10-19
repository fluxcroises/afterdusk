//! Second écran : une fenêtre sans barres qui montre la photo active (ou la
//! comparaison) sur un autre moniteur que la fenêtre principale.

use tauri::window::Color;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use super::error::CommandResult;
use crate::MAIN_WINDOW;

pub const SECONDARY_WINDOW: &str = "secondary";

/// Rectangle d'un moniteur, en pixels physiques.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Area {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && i64::from(x) < i64::from(self.x) + i64::from(self.w)
            && i64::from(y) < i64::from(self.y) + i64::from(self.h)
    }
}

/// Moniteur du second écran : le plus grand de ceux qui ne portent pas la
/// fenêtre principale ; aucun s'il n'y a qu'un écran.
pub fn pick_secondary(main_center: Option<(i32, i32)>, monitors: &[Area]) -> Option<usize> {
    let main_index = main_center.and_then(|(x, y)| monitors.iter().position(|m| m.contains(x, y)));
    monitors
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != main_index)
        .max_by_key(|(_, m)| u64::from(m.w) * u64::from(m.h))
        .map(|(i, _)| i)
        .filter(|_| monitors.len() > 1)
}

/// Ouvre le second écran, ou le ferme s'il est ouvert. Renvoie l'état final.
/// Asynchrone : sous Windows, créer une fenêtre depuis une commande synchrone
/// peut bloquer la boucle d'événements (documentation de Tauri).
#[tauri::command]
pub async fn secondary_toggle(app: AppHandle) -> CommandResult<bool> {
    if let Some(window) = app.get_webview_window(SECONDARY_WINDOW) {
        window.close()?;
        return Ok(false);
    }

    let main = app.get_webview_window(MAIN_WINDOW);
    let main_center = main.as_ref().and_then(|w| {
        let pos = w.outer_position().ok()?;
        let size = w.outer_size().ok()?;
        let half = |v: u32| i32::try_from(v / 2).unwrap_or(0);
        Some((pos.x + half(size.width), pos.y + half(size.height)))
    });
    let monitors = app.available_monitors()?;
    let areas: Vec<Area> = monitors
        .iter()
        .map(|m| Area {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        })
        .collect();

    let window =
        WebviewWindowBuilder::new(&app, SECONDARY_WINDOW, WebviewUrl::App("index.html".into()))
            .title("Afterdusk")
            .decorations(false)
            .background_color(Color(0x11, 0x12, 0x14, 0xff))
            .inner_size(1280.0, 800.0)
            .visible(false)
            .build()?;

    match pick_secondary(main_center, &areas).and_then(|i| areas.get(i)) {
        // Plein écran sur l'autre moniteur.
        Some(area) => {
            window.set_position(PhysicalPosition::new(area.x, area.y))?;
            window.set_fullscreen(true)?;
        }
        // Un seul écran : une fenêtre normale, que l'on peut déplacer.
        None => window.center()?,
    }
    window.show()?;
    // Le clavier reste à la fenêtre principale, comme dans Lightroom.
    if let Some(main) = main {
        main.set_focus()?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Area = Area {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    const RIGHT: Area = Area {
        x: 1920,
        y: 0,
        w: 2560,
        h: 1440,
    };
    const ABOVE: Area = Area {
        x: 0,
        y: -1080,
        w: 1920,
        h: 1080,
    };

    #[test]
    fn un_seul_ecran_pas_de_second_ecran() {
        assert_eq!(pick_secondary(Some((900, 500)), &[LEFT]), None);
    }

    #[test]
    fn l_autre_ecran_que_la_fenetre_principale() {
        assert_eq!(pick_secondary(Some((900, 500)), &[LEFT, RIGHT]), Some(1));
        assert_eq!(pick_secondary(Some((3000, 500)), &[LEFT, RIGHT]), Some(0));
    }

    #[test]
    fn le_plus_grand_des_autres_ecrans() {
        assert_eq!(
            pick_secondary(Some((900, 500)), &[LEFT, ABOVE, RIGHT]),
            Some(2)
        );
    }
}
