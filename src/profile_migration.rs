//! Consolidate Cikung 0.3's own feature profiles into the shared desktop profile.
//! Source profiles and the pre-migration backup remain untouched. The marker is
//! committed with the new app state, so an interrupted run cannot import twice.
use crate::{app::Preferences, tools::Toolkit};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SETTINGS: &str = "cikung-v2-settings";
const TOOLS: &str = "cikung-v2-tools";
const MARKER: &str = "cikung-single-cat-migration-v1";
const BACKUP: &str = "app.before-single-cat-v1.ron";
const MAX_PROFILE_BYTES: u64 = 32 * 1024 * 1024;
const FEATURES: [&str; 11] = [
    "agents", "music", "lyrics", "focus", "files", "tasks", "water", "notes", "chat", "services",
    "phone",
];
type Store = BTreeMap<String, String>;

struct Profile {
    values: Store,
    modified: SystemTime,
    original: Vec<u8>,
}

/// Run once, after acquiring Cikung's instance lock and before eframe opens its
/// storage. An error leaves existing app state intact and describes the file
/// needing attention. No credentials or other applications' storage are read.
pub fn migrate() -> Result<(), String> {
    let Some(folder) = eframe::storage_dir("com.cikung.desktop") else {
        return Ok(());
    };
    let sources = FEATURES
        .into_iter()
        .filter_map(|feature| {
            eframe::storage_dir(&format!("com.cikung.desktop.{feature}"))
                .map(|dir| (feature, dir.join("app.ron")))
        })
        .collect::<Vec<_>>();
    migrate_at(&folder.join("app.ron"), &sources)
}

fn migrate_at(main_path: &Path, sources: &[(&str, PathBuf)]) -> Result<(), String> {
    let main = read_profile(main_path)?;
    if main
        .as_ref()
        .is_some_and(|profile| profile.values.contains_key(MARKER))
    {
        return Ok(());
    }
    // Read and validate all inputs before preparing any replacement.
    let profiles = sources
        .iter()
        .map(|(feature, path)| read_profile(path).map(|profile| (*feature, path, profile)))
        .collect::<Result<Vec<_>, _>>()?;
    if !profiles.iter().any(|(_, _, profile)| profile.is_some()) {
        return Ok(());
    }
    let mut store = main
        .as_ref()
        .map(|profile| profile.values.clone())
        .unwrap_or_default();
    let mut settings: Preferences = decode(&store, SETTINGS, main_path)?.unwrap_or_default();
    let mut toolkit: Toolkit = decode(&store, TOOLS, main_path)?.unwrap_or_default();
    let main_time = main.as_ref().map(|profile| profile.modified);
    for (feature, path, profile) in profiles {
        let Some(profile) = profile else { continue };
        let newer = main_time.is_none_or(|time| profile.modified > time);
        let source_settings = decode::<Preferences>(&profile.values, SETTINGS, path)?;
        let source_tools = decode::<Toolkit>(&profile.values, TOOLS, path)?;
        if let Some(source) = source_tools {
            merge_tools(&mut toolkit, source, feature, newer);
        }
        if newer && let Some(source) = source_settings {
            merge_settings(&mut settings, source, feature);
        }
    }
    unique_ids(&mut toolkit)?;
    store.insert(SETTINGS.into(), encode(&settings)?);
    store.insert(TOOLS.into(), encode(&toolkit)?);
    store.insert(MARKER.into(), "1".into());
    let replacement = ron::ser::to_string_pretty(&store, Default::default())
        .map_err(|error| format!("Could not prepare Cikung's shared profile: {error}"))?;
    let folder = main_path
        .parent()
        .ok_or("The Cikung profile has no folder.")?;
    fs::create_dir_all(folder)
        .map_err(|error| format!("Could not create Cikung's profile folder: {error}"))?;
    if let Some(original) = main {
        // A previous interrupted attempt may already have the original backup.
        // Never overwrite it, or any original feature profile.
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(BACKUP))
        {
            Ok(mut backup) => backup
                .write_all(&original.original)
                .and_then(|()| backup.sync_all())
                .map_err(|error| format!("Could not back up Cikung's profile: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                // A crash during the very first backup write could leave a
                // partial file. Do not replace the main profile in that case.
                if read_profile(&folder.join(BACKUP))?.is_none() {
                    return Err("Cikung's profile backup disappeared during migration; original profiles were kept intact.".into());
                }
            }
            Err(error) => return Err(format!("Could not back up Cikung's profile: {error}")),
        }
    }
    replace_atomically(main_path, replacement.as_bytes())
}

fn read_profile(path: &Path) -> Result<Option<Profile>, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };
    if metadata.len() > MAX_PROFILE_BYTES {
        return Err(format!(
            "Cikung profile {} exceeds the 32 MB migration limit; it was kept intact.",
            path.display()
        ));
    }
    let original =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    if original.len() as u64 > MAX_PROFILE_BYTES {
        return Err(format!(
            "Cikung profile {} grew past the migration limit; it was kept intact.",
            path.display()
        ));
    }
    let values = ron::de::from_bytes(&original).map_err(|error| {
        format!(
            "Could not migrate {} because its saved state is invalid: {error}. Originals were kept intact.",
            path.display()
        )
    })?;
    Ok(Some(Profile {
        values,
        modified: metadata.modified().unwrap_or(UNIX_EPOCH),
        original,
    }))
}

fn decode<T: DeserializeOwned>(store: &Store, key: &str, path: &Path) -> Result<Option<T>, String> {
    store
        .get(key)
        .map(|value| {
            ron::from_str(value).map_err(|error| {
                format!(
                    "Could not migrate {key} from {}: {error}. Originals were kept intact.",
                    path.display()
                )
            })
        })
        .transpose()
}

fn encode(value: &impl Serialize) -> Result<String, String> {
    ron::to_string(value)
        .map_err(|error| format!("Could not prepare Cikung's saved state: {error}"))
}

fn merge_tools(target: &mut Toolkit, source: Toolkit, feature: &str, newer: bool) {
    match feature {
        "tasks" => {
            for task in source.tasks {
                if let Some(old) = target
                    .tasks
                    .iter_mut()
                    .find(|old| old.id == task.id && old.text == task.text)
                {
                    if newer {
                        *old = task;
                    }
                } else {
                    target.tasks.push(task);
                }
            }
        }
        "notes" => {
            for note in source.notes {
                if let Some(old) = target.notes.iter_mut().find(|old| {
                    old.id == note.id
                        && old.created_unix == note.created_unix
                        && (old.title == note.title || old.text == note.text)
                }) {
                    if newer {
                        *old = note;
                    }
                } else {
                    target.notes.push(note);
                }
            }
        }
        "files" => {
            for file in source.shelf {
                if let Some(old) = target.shelf.iter_mut().find(|old| old.path == file.path) {
                    if newer {
                        *old = file;
                    }
                } else {
                    target.shelf.push(file);
                }
            }
        }
        "focus" if newer => target.focus = source.focus,
        "water" if newer => target.water = source.water,
        "lyrics" if newer => target.lyrics = source.lyrics,
        _ => {}
    }
}

fn merge_settings(target: &mut Preferences, source: Preferences, feature: &str) {
    for (day, stats) in source.days {
        if !matches!(feature, "focus" | "tasks" | "water" | "notes" | "agents") {
            continue;
        }
        let current = target.days.entry(day).or_default();
        match feature {
            "focus" => current.focus = stats.focus,
            "tasks" => current.tasks = stats.tasks,
            "water" => current.water = stats.water,
            "notes" => current.notes = stats.notes,
            "agents" => current.agent_steps = stats.agent_steps,
            _ => {}
        }
    }
    match feature {
        "agents" => target.agents = source.agents,
        "chat" => target.chat = source.chat,
        "services" => target.services = source.services,
        "focus" => target.last_focus_count = source.last_focus_count,
        _ => {}
    }
}

fn unique_ids(toolkit: &mut Toolkit) -> Result<(), String> {
    let mut maximum = toolkit
        .tasks
        .iter()
        .map(|task| task.id)
        .chain(toolkit.notes.iter().map(|note| note.id))
        .max()
        .unwrap_or(0);
    let mut used = HashSet::new();
    for id in toolkit
        .tasks
        .iter_mut()
        .map(|task| &mut task.id)
        .chain(toolkit.notes.iter_mut().map(|note| &mut note.id))
    {
        if !used.insert(*id) {
            maximum = maximum.checked_add(1).ok_or(
                "Cikung's saved item IDs cannot be merged safely; original profiles were kept intact.",
            )?;
            *id = maximum;
            used.insert(*id);
        }
    }
    Ok(())
}

fn replace_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_file_name(format!(
        "app.single-cat-{}-{unique}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(windows)]
        {
            use windows::{
                Win32::Storage::FileSystem::{
                    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
                },
                core::HSTRING,
            };
            unsafe {
                MoveFileExW(
                    &HSTRING::from(temporary.as_os_str()),
                    &HSTRING::from(path.as_os_str()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            }
            .map_err(std::io::Error::other)?;
        }
        #[cfg(not(windows))]
        fs::rename(&temporary, path)?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| {
        format!(
            "Could not install Cikung's shared profile: {error}. Original profiles and the backup were kept intact."
        )
    })
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::tools::{Note, Task};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0_u8; 8];
            getrandom::fill(&mut random).unwrap();
            let folder = std::env::temp_dir().join(format!(
                "cikung-profile-migration-{:016x}",
                u64::from_le_bytes(random)
            ));
            fs::create_dir(&folder).unwrap();
            Self(folder)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn write(&self, name: &str, tools: &Toolkit, settings: &Preferences) -> PathBuf {
            let values = Store::from([
                (TOOLS.into(), encode(tools).unwrap()),
                (SETTINGS.into(), encode(settings).unwrap()),
                ("other-eframe-key".into(), "keep this".into()),
            ]);
            let path = self.path(name);
            fs::write(&path, encode(&values).unwrap()).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // This uniquely created fixture lives directly under the OS temp folder.
            if self.0.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    fn task(id: u64, text: &str, done: bool) -> Task {
        Task {
            id,
            text: text.into(),
            done,
            due_unix: None,
            due_notified: false,
            google_event: None,
        }
    }
    fn note(id: u64, title: &str, text: &str, created_unix: u64) -> Note {
        Note {
            id,
            title: title.into(),
            text: text.into(),
            created_unix,
        }
    }

    #[test]
    fn merges_conflicts_without_losing_rows_and_future_ids_remain_unique() {
        let mut target = Toolkit::default();
        target.tasks.push(task(1, "workspace", false));
        target.tasks.push(task(2, "shared", false));
        target.notes.push(note(3, "Meeting", "Old", 42));
        let mut source = Toolkit::default();
        source.tasks = vec![task(1, "feature", false), task(2, "shared", true)];
        merge_tools(&mut target, source, "tasks", true);
        let mut source = Toolkit::default();
        source.notes = vec![note(3, "Meeting", "Edited", 42), note(1, "New", "New", 50)];
        merge_tools(&mut target, source, "notes", true);
        unique_ids(&mut target).unwrap();
        assert_eq!(target.tasks.len(), 3);
        assert!(target.tasks[1].done);
        assert_eq!(target.notes.len(), 2);
        assert_eq!(target.notes[0].text, "Edited");
        let ids = target
            .tasks
            .iter()
            .map(|task| task.id)
            .chain(target.notes.iter().map(|note| note.id))
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 5);
        let next = target.add_task("after migration").unwrap();
        assert!(!ids.contains(&next));
    }

    #[test]
    fn feature_merge_keeps_common_preferences_and_uses_owned_settings_only() {
        let mut main = Preferences::default();
        main.shortcut = "Z".into();
        main.always_on_top = false;
        let mut agents = Preferences::default();
        agents.agents.project = "D:/agent project".into();
        agents.shortcut = "A".into();
        merge_settings(&mut main, agents, "agents");
        assert_eq!(main.agents.project, "D:/agent project");
        assert_eq!(main.shortcut, "Z");
        assert!(!main.always_on_top);
        let mut toolkit = Toolkit::default();
        toolkit.focus.focus_minutes = 40;
        let mut source = Toolkit::default();
        source.focus.focus_minutes = 10;
        merge_tools(&mut toolkit, source, "focus", false);
        assert_eq!(toolkit.focus.focus_minutes, 40);
        let mut source = Toolkit::default();
        source.focus.focus_minutes = 15;
        merge_tools(&mut toolkit, source, "focus", true);
        assert_eq!(toolkit.focus.focus_minutes, 15);
    }

    #[test]
    fn commits_marker_and_backup_once_preserving_source_and_outer_keys() {
        let fixture = Fixture::new();
        let main = fixture.write("app.ron", &Toolkit::default(), &Preferences::default());
        let original = fs::read(&main).unwrap();
        let mut source_tools = Toolkit::default();
        source_tools.add_task("From feature").unwrap();
        let source = fixture.write("tasks.ron", &source_tools, &Preferences::default());
        let source_bytes = fs::read(&source).unwrap();
        migrate_at(&main, &[("tasks", source.clone())]).unwrap();
        let migrated = fs::read(&main).unwrap();
        let saved = read_profile(&main).unwrap().unwrap();
        assert_eq!(saved.values[MARKER], "1");
        assert_eq!(saved.values["other-eframe-key"], "keep this");
        assert_eq!(
            decode::<Toolkit>(&saved.values, TOOLS, &main)
                .unwrap()
                .unwrap()
                .tasks
                .len(),
            1
        );
        assert_eq!(fs::read(fixture.path(BACKUP)).unwrap(), original);
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        fs::write(&source, "now invalid").unwrap();
        migrate_at(&main, &[("tasks", source)]).unwrap();
        assert_eq!(fs::read(&main).unwrap(), migrated);
        assert_eq!(fs::read(fixture.path(BACKUP)).unwrap(), original);
    }

    #[test]
    fn malformed_input_never_replaces_original_or_creates_backup() {
        let fixture = Fixture::new();
        let main = fixture.write("app.ron", &Toolkit::default(), &Preferences::default());
        let before = fs::read(&main).unwrap();
        let source = fixture.path("tasks.ron");
        fs::write(&source, "invalid saved state").unwrap();
        let error = migrate_at(&main, &[("tasks", source)]).unwrap_err();
        assert!(error.contains("tasks.ron"));
        assert_eq!(fs::read(&main).unwrap(), before);
        assert!(!fixture.path(BACKUP).exists());
    }

    #[test]
    fn missing_workspace_can_import_feature_data_without_inventing_a_backup() {
        let fixture = Fixture::new();
        let main = fixture.path("app.ron");
        let mut tools = Toolkit::default();
        tools.focus.focus_minutes = 35;
        let source = fixture.write("focus.ron", &tools, &Preferences::default());
        migrate_at(&main, &[("focus", source)]).unwrap();
        let saved = read_profile(&main).unwrap().unwrap();
        assert_eq!(
            decode::<Toolkit>(&saved.values, TOOLS, &main)
                .unwrap()
                .unwrap()
                .focus
                .focus_minutes,
            35
        );
        assert!(!fixture.path(BACKUP).exists());
    }
}
