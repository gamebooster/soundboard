use iced::{
    executor, Executor, Align, Application, button, Button, Container, Column, Row, Command, Element,
    Settings, Subscription, Text, HorizontalAlignment, Length,
};

/*
 * GUI SETTINGS 
 */
const WINDOW_HEIGHT: u32 = 700;
const WINDOW_WIDTH: u32 = 600;
const WINDOW_TITLE: &str = "Soundboard";

const BUTTON_DEFAULT_NAME: &str = "Button";

const DEFAULT_SPACING: u16 = 0x10;


pub fn main() {
    let mut settings = Settings::default();
    settings.window.size = (WINDOW_WIDTH, WINDOW_HEIGHT);
    settings.window.resizable = false;
    Soundboard::run(settings);
}

#[derive(Default)]
struct Soundboard {
    play_sound: button::State,
    stop_sound: button::State,
    mute_sound_global: button::State,
    mute_sound_local: button::State,
    
    delete_profile: button::State,
    create_new_profile: button::State,
    edit_profile: button::State,
    set_active_profile: button::State,

    open_settings: button::State,

    status_text: String,
}

enum State {
    Idle,               // no sound playing
    SoundPlaying,       // sound currently playing
    SoundPause          // sound was paused while playing
}

#[derive(Debug, Clone, Copy)]
enum Message {
    /* SOUND */
    PlaySound,          // play new sound
    PauseSound,         // pause current sound
    ContinueSound,      // continue paused sound
    StopSound,          // stop current sound
    MuteLocalSound,     // mute playback locally
    MuteGlobalSound,    // mute playback globally

    // PROFILES
    DeleteProfile,      // delete current profile
    CreateProfile,      // add new profile
    EditProfile,        // edit profilename
    SetProfile,         // sets the current active profile

    /* SETTINGS */
    OpenSettings        // open settings page
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Soundboard, Command<Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        WINDOW_TITLE.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PlaySound          => { println!("Start playing sound..."); }
            Message::PauseSound         => { println!("Pause current sound..."); }
            Message::ContinueSound      => { println!("Continue current sound..."); }
            Message::StopSound          => { println!("Stop current sound..."); }
            Message::MuteLocalSound     => { println!("Mute local playback..."); }
            Message::MuteGlobalSound    => { println!("Mute global playback..."); }
            Message::DeleteProfile      => { println!("Delete current profile..."); }
            Message::CreateProfile      => { println!("Create new profile..."); }
            Message::EditProfile        => { println!("Edit current profile..."); }
            Message::SetProfile         => { println!("Select new active profile..."); }
            Message::OpenSettings       => { println!("Open settings..."); }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        /* ========================================
         * templates
         * ======================================== */

        let button = |state, label| {
            Button::new(
                state,
                Text::new(label)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .min_width(80)
            .min_height(40)
            .padding(0)
            //.style(style)
        };

        /* ========================================
         * buttons
         * ======================================== */

        let play_sound_button = button(&mut self.play_sound, "Play")
            .on_press(Message::PlaySound);
        
        let stop_sound_button = button(&mut self.stop_sound, "Stop")
            .on_press(Message::StopSound);
        
        let mute_sound_local_button = button(&mut self.mute_sound_local, "Mute L")
            .on_press(Message::MuteLocalSound);

        let mute_sound_global_button = button(&mut self.mute_sound_global, "Mute G")
            .on_press(Message::MuteGlobalSound);

        let delete_profile_button = button(&mut self.delete_profile, "x")
            .on_press(Message::DeleteProfile);

        let create_new_profile_button = button(&mut self.create_new_profile, "+")
            .on_press(Message::CreateProfile);
        
        let edit_profile_button = button(&mut self.edit_profile, "o")
            .on_press(Message::EditProfile);
        
        let set_profile_button = button(&mut self.set_active_profile, "set")
            .on_press(Message::SetProfile);

        let open_settings_button = button(&mut self.open_settings, "Settings")
            .on_press(Message::OpenSettings);
            
        /* ========================================
         * components
         * ======================================== */

        // control elements for current playing sound
        let playback_control = Row::new()
            .spacing(DEFAULT_SPACING)
            .push(play_sound_button)
            .push(stop_sound_button)
            .push(mute_sound_local_button)
            .push(mute_sound_global_button)
            .push(open_settings_button);

        // info elements for current playing sound
        let playback_info = Row::new()
            .spacing(DEFAULT_SPACING);

        // sound search and hotkey profile selection
        let search_and_profile = Row::new()
            .spacing(DEFAULT_SPACING)
            .push(create_new_profile_button)
            .push(edit_profile_button)
            .push(delete_profile_button);
        
        // list of sounds and active hotkeys
        let sounds_and_hotkeys = Row::new()
            .spacing(DEFAULT_SPACING)
            .push(set_profile_button);

        // final content
        let content = Column::new()
            .align_items(Align::Start)
            .spacing(DEFAULT_SPACING)
            .push(playback_control)
            .push(playback_info)
            .push(search_and_profile)
            .push(sounds_and_hotkeys);

        /* ========================================
         * final
         * ======================================== */

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
