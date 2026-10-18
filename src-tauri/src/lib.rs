pub mod commands;
pub mod db;
pub mod import;
pub mod library;
pub mod metadata;
pub mod raw;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Afterdusk n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
