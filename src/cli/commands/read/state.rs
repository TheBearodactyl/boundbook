use {
    miette::IntoDiagnostic,
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    },
    xxhash_rust::xxh3::xxh3_64,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookState {
    pub current_page: usize,
    #[serde(default)]
    pub bookmarks: BTreeSet<usize>,
    #[serde(default)]
    pub source_path: String,
}

fn saves_directory() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("boundbook").join("saves"))
}

fn save_filename(book_path: &Path) -> String {
    let canonical = book_path
        .canonicalize()
        .unwrap_or_else(|_| book_path.to_path_buf());
    let bytes = canonical.to_string_lossy().as_bytes().to_vec();
    let hash = xxh3_64(&bytes);
    format!("{hash:016x}.toml")
}

fn save_path(book_path: &Path) -> Option<PathBuf> {
    saves_directory().map(|dir| dir.join(save_filename(book_path)))
}

pub fn load_state(book_path: &Path) -> BookState {
    let Some(path) = save_path(book_path) else {
        return BookState::default();
    };

    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => BookState::default(),
    }
}

pub fn save_state(book_path: &Path, state: &BookState) -> miette::Result<()> {
    let dir =
        saves_directory().ok_or_else(|| miette::miette!("Could not determine data directory"))?;
    fs::create_dir_all(&dir).into_diagnostic()?;

    let path = dir.join(save_filename(book_path));
    let serialized = toml::to_string_pretty(state).into_diagnostic()?;
    fs::write(&path, serialized).into_diagnostic()?;

    Ok(())
}
