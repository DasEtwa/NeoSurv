use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::world::scene::Scene;

pub(crate) const SCENE_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SceneFile {
    pub(crate) version: u32,
    pub(crate) scene: Scene,
}

pub(crate) fn to_ron(scene: &Scene) -> Result<String> {
    let payload = SceneFile {
        version: SCENE_FILE_VERSION,
        scene: scene.clone(),
    };

    ron::ser::to_string_pretty(&payload, ron::ser::PrettyConfig::new()).with_context(|| {
        format!(
            "failed to serialize scene '{}' into RON envelope version {}",
            scene.name, SCENE_FILE_VERSION
        )
    })
}

pub(crate) fn from_ron(input: &str) -> Result<Scene> {
    match ron::from_str::<SceneFile>(input) {
        Ok(file) => {
            if file.version != SCENE_FILE_VERSION {
                bail!(
                    "unsupported scene file version {} (expected {})",
                    file.version,
                    SCENE_FILE_VERSION
                );
            }

            Ok(file.scene)
        }
        Err(envelope_err) => {
            // Backward compatibility with the old pre-envelope format.
            ron::from_str::<Scene>(input).with_context(|| {
                format!(
                    "failed to parse versioned SceneFile envelope ({envelope_err}); also failed to parse legacy Scene format"
                )
            })
        }
    }
}

pub(crate) fn save_scene_to_path(path: impl AsRef<Path>, scene: &Scene) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create scene directory {}", parent.display()))?;
    }

    let ron = to_ron(scene).with_context(|| {
        format!(
            "failed to serialize scene '{}' before writing {}",
            scene.name,
            path.display()
        )
    })?;

    atomic_write(path, ron.as_bytes()).with_context(|| {
        format!(
            "failed to atomically write scene '{}' to {}",
            scene.name,
            path.display()
        )
    })?;

    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut attempt = 0_u32;
    loop {
        let temp_path = temp_file_path(path, attempt);
        attempt += 1;

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                if let Err(err) = write_and_sync(&mut file, bytes)
                    .and_then(|_| replace_via_rename(&temp_path, path))
                    .and_then(|_| sync_directory(directory))
                {
                    let _ = fs::remove_file(&temp_path);
                    return Err(err).with_context(|| {
                        format!(
                            "failed to persist temporary file {} as {}",
                            temp_path.display(),
                            path.display()
                        )
                    });
                }

                return Ok(());
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && attempt < 16 => {
                continue;
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to create temporary scene file in {}",
                        directory.display()
                    )
                });
            }
        }
    }
}

fn replace_via_rename(temp_path: &Path, destination_path: &Path) -> Result<()> {
    match fs::rename(temp_path, destination_path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            #[cfg(windows)]
            {
                if rename_err.kind() == std::io::ErrorKind::AlreadyExists {
                    let _ = fs::remove_file(destination_path);
                    fs::rename(temp_path, destination_path)?;
                    return Ok(());
                }
            }

            Err(rename_err.into())
        }
    }
}

fn write_and_sync(file: &mut File, bytes: &[u8]) -> Result<()> {
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

fn temp_file_path(path: &Path, attempt: u32) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("scene");

    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    parent.join(format!(
        ".{}.tmp-{}-{}-{}",
        file_name,
        std::process::id(),
        now_nanos,
        attempt
    ))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

pub(crate) fn load_scene_from_path(path: impl AsRef<Path>) -> Result<Scene> {
    let path = path.as_ref();

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read scene file {}", path.display()))?;

    from_ron(&content).with_context(|| format!("failed to deserialize {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{SCENE_FILE_VERSION, SceneFile, from_ron};
    use crate::world::scene::Scene;

    #[test]
    fn from_ron_accepts_legacy_scene_format() {
        let scene = Scene::demo();
        let legacy_ron = ron::ser::to_string_pretty(&scene, ron::ser::PrettyConfig::new())
            .expect("legacy scene should serialize");

        let parsed = from_ron(&legacy_ron).expect("legacy scene should parse");

        assert_eq!(parsed, scene);
    }

    #[test]
    fn from_ron_rejects_unsupported_version() {
        let unsupported = SceneFile {
            version: SCENE_FILE_VERSION + 1,
            scene: Scene::demo(),
        };

        let ron = ron::ser::to_string_pretty(&unsupported, ron::ser::PrettyConfig::new())
            .expect("unsupported envelope should serialize");

        let err = from_ron(&ron).expect_err("unsupported version should fail");
        let err_text = err.to_string();

        assert!(err_text.contains("unsupported scene file version"));
        assert!(err_text.contains(&(SCENE_FILE_VERSION + 1).to_string()));
    }
}
