//! Loading a whole checkout of the data at once.

use crate::{BossScenariosFile, ConnectionFile, EnemiesFile, HelpersFile, ItemsFile, NumericsFile, Room, TechFile, WeaponsFile};
use serde::de::DeserializeOwned;
use std::fmt;
use std::path::{Path, PathBuf};

/// Every typed file in an `sm-json-data` checkout.
#[derive(Clone, Debug, PartialEq)]
pub struct GameData {
    pub items: ItemsFile,
    pub tech: TechFile,
    pub helpers: HelpersFile,
    pub numerics: NumericsFile,
    pub weapons: WeaponsFile,
    pub enemies: EnemiesFile,
    pub bosses: EnemiesFile,
    pub boss_scenarios: BossScenariosFile,
    pub connections: Vec<ConnectionFile>,
    pub rooms: Vec<Room>,
}

impl GameData {
    /// Loads a checkout, stopping at the first file that fails to read or parse.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, LoadError> {
        let root = root.as_ref();
        Ok(Self {
            items: load_file(root.join("items.json"))?,
            tech: load_file(root.join("tech.json"))?,
            helpers: load_file(root.join("helpers.json"))?,
            numerics: load_file(root.join("numerics.json"))?,
            weapons: load_file(root.join("weapons/main.json"))?,
            enemies: load_file(root.join("enemies/main.json"))?,
            bosses: load_file(root.join("enemies/bosses/main.json"))?,
            boss_scenarios: load_file(root.join("enemies/bosses/scenarios.json"))?,
            connections: connection_paths(root)?.into_iter().map(load_file).collect::<Result<_, _>>()?,
            rooms: room_paths(root)?.into_iter().map(load_file).collect::<Result<_, _>>()?,
        })
    }
}

/// Reads and parses one JSON file, tolerating a leading byte order mark.
pub fn load_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, LoadError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|source| LoadError::Parse {
        path: path.to_owned(),
        source,
    })
}

/// Lists room files in sorted order, leaving out the `roomDiagrams.json` layout files.
pub fn room_paths(root: impl AsRef<Path>) -> Result<Vec<PathBuf>, LoadError> {
    let mut paths = json_files(&root.as_ref().join("region"))?;
    paths.retain(|path| path.file_name().is_some_and(|name| name != "roomDiagrams.json"));
    Ok(paths)
}

/// Lists connection files in sorted order.
pub fn connection_paths(root: impl AsRef<Path>) -> Result<Vec<PathBuf>, LoadError> {
    json_files(&root.as_ref().join("connection"))
}

fn json_files(directory: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let io_error = |source| LoadError::Io {
        path: directory.to_owned(),
        source,
    };
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        if path.is_dir() {
            paths.extend(json_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// A failure to read or parse a data file.
#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, source: serde_json::Error },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            LoadError::Parse { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Io { source, .. } => Some(source),
            LoadError::Parse { source, .. } => Some(source),
        }
    }
}
