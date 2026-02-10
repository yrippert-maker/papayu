//! Domain notes: curated short notes from online research, stored per project.
//!
//! File: `.papa-yu/notes/domain_notes.json`
//! Env: PAPAYU_NOTES_MAX_ITEMS, PAPAYU_NOTES_MAX_CHARS_PER_NOTE, PAPAYU_NOTES_MAX_TOTAL_CHARS, PAPAYU_NOTES_TTL_DAYS

mod distill;
mod selection;
mod storage;

pub use distill::distill_and_save_note;
pub use selection::get_notes_block_for_prompt;
pub use storage::{
    clear_expired_notes, delete_note, load_domain_notes, pin_note, save_domain_notes, DomainNote,
    DomainNotes, NoteSource,
};
