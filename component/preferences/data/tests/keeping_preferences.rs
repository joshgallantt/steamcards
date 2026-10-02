//! The preferences as the config file keeps them: the same fields, in the
//! same places, as every version of steamcards has written them.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use config_file::ConfigFile;
use game::AppId;
use preferences::Preferences;
use preferences_data::{FilePreferencesStore, PreferencesStore};
use serde_json::{Value, json};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "steamcards-preferences-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir.join("config.json")
}

fn store(path: &Path) -> FilePreferencesStore {
    FilePreferencesStore::new(Arc::new(ConfigFile::open(path.to_path_buf()).unwrap()))
}

#[test]
fn preferences_are_kept_at_the_top_of_the_config_file() {
    let path = temp("shape");
    store(&path)
        .save(&Preferences {
            priority_games: vec![AppId(620), AppId(440)],
            skipped_games: vec![AppId(730)],
            only_priority: true,
            appear_online: false,
            restart_games: true,
            auto_update: false,
            hours_before_drops: 2,
            skip_private: false,
            skip_refundable: true,
        })
        .unwrap();

    let json: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        json,
        json!({
            "priority_games": [620, 440],
            "skipped_games": [730],
            "only_priority": true,
            "appear_online": false,
            "restart_games": true,
            "auto_update": false,
            "hours_before_drops": 2,
            "skip_private": false,
            "skip_refundable": true,
        })
    );
}

#[test]
fn preferences_saved_before_read_as_they_were() {
    let path = temp("before");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{"steam":{"refresh_token":"t","account_name":"me","steam_id":"76561197960287930","login_id":7},"priority_games":[620],"only_priority":true}"#,
    )
    .unwrap();

    let read = store(&path).preferences();
    assert_eq!(
        read,
        Preferences {
            priority_games: vec![AppId(620)],
            only_priority: true,
            ..Default::default()
        }
    );
    assert!(
        read.auto_update,
        "a file from before it was a choice keeps steamcards up to date"
    );
    assert_eq!(read.hours_before_drops, 3, "as before it was a choice");
    assert!(read.skip_private && read.skip_refundable);
}

#[test]
fn more_hours_than_theres_a_choice_of_are_the_most() {
    let path = temp("too-many-hours");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, r#"{"hours_before_drops":48}"#).unwrap();
    assert_eq!(
        store(&path).preferences().hours_before_drops,
        Preferences::MOST_HOURS_BEFORE_DROPS
    );
}

#[test]
fn preferences_that_dont_read_are_the_defaults() {
    let path = temp("unreadable");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, r#"{"priority_games":"620"}"#).unwrap();
    assert_eq!(store(&path).preferences(), Preferences::default());
}
