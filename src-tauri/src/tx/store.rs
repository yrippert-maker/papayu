//! Undo/redo stack in userData/history/state.json

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::history_dir;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UndoRedoStateFile {
    pub undo_stack: Vec<String>,
    pub redo_stack: Vec<String>,
}

fn state_path(app: &AppHandle) -> PathBuf {
    history_dir(app).join("state.json")
}

fn load_state(app: &AppHandle) -> UndoRedoStateFile {
    let p = state_path(app);
    if let Ok(bytes) = fs::read(&p) {
        if let Ok(s) = serde_json::from_slice::<UndoRedoStateFile>(&bytes) {
            return s;
        }
    }
    UndoRedoStateFile::default()
}

fn save_state(app: &AppHandle, state: &UndoRedoStateFile) -> io::Result<()> {
    let p = state_path(app);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(p, bytes)
}

pub fn push_undo(app: &AppHandle, tx_id: String) -> io::Result<()> {
    let mut state = load_state(app);
    state.undo_stack.push(tx_id);
    state.redo_stack.clear();
    save_state(app, &state)
}

pub fn pop_undo(app: &AppHandle) -> Option<String> {
    let mut state = load_state(app);
    let tx_id = state.undo_stack.pop()?;
    save_state(app, &state).ok()?;
    Some(tx_id)
}

pub fn push_redo(app: &AppHandle, tx_id: String) -> io::Result<()> {
    let mut state = load_state(app);
    state.redo_stack.push(tx_id);
    save_state(app, &state)
}

pub fn pop_redo(app: &AppHandle) -> Option<String> {
    let mut state = load_state(app);
    let tx_id = state.redo_stack.pop()?;
    save_state(app, &state).ok()?;
    Some(tx_id)
}

pub fn clear_redo(app: &AppHandle) -> io::Result<()> {
    let mut state = load_state(app);
    state.redo_stack.clear();
    save_state(app, &state)
}

pub fn get_undo_redo_state(app: &AppHandle) -> (bool, bool) {
    let state = load_state(app);
    (!state.undo_stack.is_empty(), !state.redo_stack.is_empty())
}
