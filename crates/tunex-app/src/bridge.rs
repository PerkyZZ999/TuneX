//! Bridge modules: `QObject` row stores, one file per bridged type.
//!
//! Exception (load-bearing): all CXX-Qt bridges share one module
//! ([`models`]) because cxx-qt emits a single `QAbstractListModel`
//! downcast shim per bridge — sibling bridges subclassing the same Qt base
//! collide at link time. Per-model rows, loaders, and tests stay in their
//! own files; only the `#[cxx_qt::bridge]` declarations are shared.

pub mod album_list_model;
pub mod artist_list_model;
pub mod library_track_model;
pub mod models;
pub mod track_list_model;

#[cfg(test)]
pub mod test_support {
    //! Shared browse-model fixtures: one seeded throwaway index for every
    //! loader test (two artists, two albums, three tracks).

    /// Scratch directory removed on scope exit.
    #[derive(Debug)]
    pub struct Guard(pub std::path::PathBuf);

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Seed a throwaway file index; returns the cleanup guard plus the db path.
    ///
    /// # Panics
    ///
    /// Panics on fixture setup failure (directory, database, or upsert) —
    /// tests cannot proceed without the seed.
    #[must_use]
    pub fn seeded_index(case: &str) -> (Guard, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("tunex-browse-seed-{case}-{}", std::process::id()));
        // Parallel-safe: one dir per case; nextest never runs the same test twice.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        let path = dir.join("library.db");
        let mut db = tunex_library::open_file(&path).expect("seed opens");
        for (name, title, artist, album) in [
            ("a1.flac", "One", "Nova Rae", "Night Tapes"),
            ("a2.flac", "Two", "Nova Rae", "Night Tapes"),
            ("b1.flac", "Solo", "Solo Act", "Only"),
        ] {
            tunex_library::upsert_track(
                &mut db,
                &tunex_library::NewTrack {
                    path: format!("/music/{name}"),
                    stable_key: name.to_owned(),
                    title: Some(title.to_owned()),
                    artist: Some(artist.to_owned()),
                    album: Some(album.to_owned()),
                    ..Default::default()
                },
            )
            .expect("seed upserts");
        }
        (Guard(dir), path)
    }
}
