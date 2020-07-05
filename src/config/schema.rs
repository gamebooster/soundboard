table! {
    settings (name) {
        name -> Nullable<Text>,
        value -> Nullable<Text>,
        description -> Nullable<Text>,
    }
}

table! {
    soundboard_sound (soundboard_id, sound_id) {
        soundboard_id -> Integer,
        sound_id -> Integer,
    }
}

table! {
    soundboards (id) {
        id -> Nullable<Integer>,
        name -> Text,
        path -> Text,
        hotkey -> Nullable<Text>,
        position -> Nullable<Integer>,
        disabled -> Nullable<Integer>,
    }
}

table! {
    sounds (id) {
        id -> Integer,
        name -> Text,
        path -> Text,
        hotkey -> Nullable<Text>,
        headers -> Nullable<Text>,
    }
}

joinable!(soundboard_sound -> soundboards (soundboard_id));
joinable!(soundboard_sound -> sounds (sound_id));

allow_tables_to_appear_in_same_query!(settings, soundboard_sound, soundboards, sounds,);
