use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FolderLinks {
    pub paths: Vec<String>,
}

const FILENAME: &str = "folder_links.json";

pub fn load_folder_links(app_data_dir: &Path) -> FolderLinks {
    let p = app_data_dir.join(FILENAME);
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(links) = serde_json::from_str::<FolderLinks>(&s) {
            return links;
        }
    }
    FolderLinks::default()
}

pub fn save_folder_links(app_data_dir: &Path, links: &FolderLinks) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let p = app_data_dir.join(FILENAME);
    fs::write(
        &p,
        serde_json::to_string_pretty(links).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
