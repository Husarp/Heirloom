//! Heirloom core: the family archive folder (GEDCOM + small `.heirloom` files), its change history
//! and the rebuildable search cache (PLAN.md §5 and §11.2).

pub mod archive;
pub mod cache;
pub mod fold;
pub mod gedcom;
pub mod history;
pub mod polish;

pub use archive::{Archive, Edit, SaveOptions, SaveReport, Settings};
pub use cache::{Cache, PersonHit};

/// What Heirloom writes into HEAD.SOUR. A file with another value there came from another program.
pub const PRODUCT_ID: &str = "HEIRLOOM";

/// Errors shown to the user, so the messages are in Polish like the rest of the interface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Nie można odczytać ani zapisać pliku: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plik ustawień Heirlooma jest uszkodzony: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Błąd bazy podręcznej: {0}")]
    Cache(#[from] rusqlite::Error),
    #[error("W tym folderze nie ma pliku GEDCOM (.ged).")]
    NoDataFile,
    #[error("W tym folderze jest kilka plików GEDCOM — wybierz jeden: {}", .0.join(", "))]
    SeveralDataFiles(Vec<String>),
    #[error("Tu już jest archiwum: {}", .0.display())]
    AlreadyExists(std::path::PathBuf),
    #[error("To archiwum jest tylko do odczytu.")]
    ReadOnly,
    #[error("Ten plik pochodzi z programu {0}. Zapis do niego wymaga potwierdzenia.")]
    ForeignNeedsConfirmation(String),
    #[error("Plik danych został zmieniony w innym programie.")]
    Conflict,
    #[error("Nie ma rekordu {0}.")]
    NoSuchRecord(String),
    #[error("Rekord {0} już istnieje.")]
    DuplicateRecord(String),
    #[error("Rekord bez identyfikatora (@…@) nie może być dodany ani zmieniony.")]
    MissingXref,
}

pub type Result<T> = std::result::Result<T, Error>;
