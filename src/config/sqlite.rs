use diesel::deserialize::Queryable;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};

use super::schema::*;

#[derive(Queryable)]
pub struct Soundboard {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub hotkey: Option<String>,
    pub position: Option<i32>,
    pub disabled: Option<i32>,
}

#[derive(Queryable)]
pub struct Sound {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub hotkey: Option<String>,
    pub headers: Option<String>,
}

#[derive(Insertable)]
#[table_name = "soundboards"]
pub struct NewSoundboard<'a> {
    pub id: i32,
    pub name: &'a str,
    pub path: &'a str,
    pub hotkey: Option<&'a String>,
    pub position: Option<&'a i32>,
    pub disabled: Option<&'a i32>,
}

#[derive(Insertable)]
#[table_name = "sounds"]
pub struct NewSound<'a> {
    pub id: i32,
    pub name: &'a str,
    pub path: &'a str,
    pub hotkey: Option<&'a String>,
    pub headers: Option<String>,
}

#[derive(Insertable)]
#[table_name = "soundboard_sound"]
pub struct NewSoundboardSound<'a> {
    pub soundboard_id: &'a i32,
    pub sound_id: &'a i32,
}

fn get_database_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("soundboard.sqlite");
    path
}

fn establish_connection() -> Result<SqliteConnection> {
    SqliteConnection::establish(get_database_path().to_str().unwrap())
        .context("failed to establish sqlite connection")
}

fn create_soundboard<'a>(conn: &SqliteConnection, soundboard: &'a NewSoundboard) {
    use super::schema::soundboards;

    diesel::insert_into(soundboards::table)
        .values(soundboard)
        .execute(conn)
        .expect("failed to insert soundboard");
}

fn create_sound<'a>(conn: &SqliteConnection, soundboard_id: i32, sounds: &'a [NewSound]) {
    use super::schema::soundboard_sound;
    use super::schema::sounds;

    diesel::insert_into(sounds::table)
        .values(sounds)
        .execute(conn)
        .expect("failed to insert sound");

    let mut new_relation = Vec::new();
    for sound in sounds {
        let new_soundboard_sound = NewSoundboardSound {
            soundboard_id: &soundboard_id,
            sound_id: &sound.id,
        };
        new_relation.push(new_soundboard_sound);
    }

    diesel::insert_into(soundboard_sound::table)
        .values(&new_relation)
        .execute(conn)
        .expect("failed to insert soundboard_sound");
}

pub fn import_config(config: &crate::config::MainConfig) -> Result<()> {
    if get_database_path().exists() {
        std::fs::remove_file(get_database_path()).expect("failed to remove old database");
    }
    let connection = establish_connection()?;

    crate::embedded_migrations::run(&connection)?;

    let mut sound_id = 0;

    for (id, soundboard) in config.soundboards.iter().enumerate() {
        let disabled: Option<i32> = {
            if let Some(true) = soundboard.disabled {
                Some(1)
            } else {
                None
            }
        };
        let position: Option<i32> = {
            if let Some(pos) = soundboard.position {
                Some(pos as i32)
            } else {
                None
            }
        };
        let new_soundboard = NewSoundboard {
            id: id as i32,
            name: &soundboard.name,
            path: &soundboard.path,
            hotkey: soundboard.hotkey.as_ref(),
            position: position.as_ref(),
            disabled: disabled.as_ref(),
        };
        create_soundboard(&connection, &new_soundboard);

        let mut new_sounds = Vec::new();
        for sound in soundboard.sounds.as_ref().unwrap() {
            let header_string = {
                if let Some(headers) = &sound.headers {
                    Some(toml::to_string(headers).unwrap())
                } else {
                    None
                }
            };
            let new_sound = NewSound {
                id: sound_id,
                name: &sound.name,
                path: &sound.path,
                hotkey: sound.hotkey.as_ref(),
                headers: header_string,
            };
            new_sounds.push(new_sound);
            sound_id += 1;
        }
        create_sound(&connection, id as i32, &new_sounds);
    }

    Ok(())
}
