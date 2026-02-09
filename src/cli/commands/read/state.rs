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

#[cfg(test)]
mod tests {
    #![allow(unused, clippy::missing_panics_doc)]
    use {super::*, assert2::check as assert, std::collections::BTreeSet};

    #[test]
    fn test_save_filename_is_deterministic() {
        let path = Path::new("some/book.bbf");
        let f1 = save_filename(path);
        let f2 = save_filename(path);
        assert!(f1 == f2);
    }

    #[test]
    fn test_save_filename_differs_for_different_paths() {
        let f1 = save_filename(Path::new("book_a.bbf"));
        let f2 = save_filename(Path::new("book_b.bbf"));
        assert!(f1 != f2);
    }

    #[test]
    #[macroni_n_cheese::mathinator2000]
    fn test_save_filename_format_is_hex_toml() {
        let f = save_filename(Path::new("test.bbf"));
        assert!(f.ends_with(".toml"));
        let hex_part = &f[..f.len() - 5];
        assert!(hex_part.len() == 16);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_bookstate_default_values() {
        let state = BookState::default();
        assert!(state.current_page == 0);
        assert!(state.bookmarks.is_empty());
        assert!(state.source_path.is_empty());
    }

    #[test]
    fn test_bookstate_serialization_roundtrip() {
        let mut bookmarks = BTreeSet::new();
        bookmarks.insert(5);
        bookmarks.insert(10);
        bookmarks.insert(42);

        let state = BookState {
            current_page: 15,
            bookmarks,
            source_path: "/home/user/manga.bbf".to_string(),
        };

        let serialized = toml::to_string_pretty(&state).unwrap();
        let deserialized: BookState = toml::from_str(&serialized).unwrap();

        assert!(deserialized.current_page == 15);
        assert!(deserialized.bookmarks.len() == 3);
        assert!(deserialized.bookmarks.contains(&5));
        assert!(deserialized.bookmarks.contains(&10));
        assert!(deserialized.bookmarks.contains(&42));
        assert!(deserialized.source_path == "/home/user/manga.bbf");
    }

    #[test]
    fn test_bookstate_deserialize_missing_fields_uses_defaults() {
        let toml_str = "current_page = 7\n";
        let state: BookState = toml::from_str(toml_str).unwrap();
        assert!(state.current_page == 7);
        assert!(state.bookmarks.is_empty());
        assert!(state.source_path.is_empty());
    }

    #[test]
    fn test_load_state_nonexistent_path_returns_default() {
        let state = load_state(Path::new("nonexistent_file_that_does_not_exist_12345.bbf"));
        assert!(state.current_page == 0);
        assert!(state.bookmarks.is_empty());
    }

    #[test]
    fn test_save_and_load_state_roundtrip() {
        let mut bookmarks = BTreeSet::new();
        bookmarks.insert(3);
        bookmarks.insert(7);

        let state = BookState {
            current_page: 42,
            bookmarks,
            source_path: "roundtrip_test.bbf".to_string(),
        };

        let book_path = Path::new("__test_save_load_roundtrip_unique_path_12345.bbf");

        if save_state(book_path, &state).is_ok() {
            let loaded = load_state(book_path);
            assert!(loaded.current_page == 42);
            assert!(loaded.bookmarks.contains(&3));
            assert!(loaded.bookmarks.contains(&7));
            assert!(loaded.source_path == "roundtrip_test.bbf");

            if let Some(path) = save_path(book_path) {
                let _ = fs::remove_file(path);
            }
        }
    }
}
