---
marp: true
header: rust project 2020
footer: https://github.com/gamebooster/soundboard
---

# soundboard

cross-platform desktop application to spice up your audio/video conferences

---

# Initial requirements

- play local file sounds on a button press over the microphone and one output
- supported formats: mp3, wav
- cross-platform
- global hotkey support
- readable config format (toml, json etc)
- native gui

---

# Demo!

---

# Current status

- all initial requirements implemented
- supported formats: mp3, wav, ogg, flac, xm
- config file format: toml
- extra features:
  - http links as source (download)
  - multiple soundboards
  - http-api/webui with drag/drop and mobile support
  - telegram bot
- automatic continuous builds via Github Action
- python scrapers for myinstants.com, soundboard.com, 101soundboards.com
- first alpha release v0.0.2 on crates.io and packaged as binary release

---

# Config file example

`soundboards\favorites.toml`
````
name = 'favorites'
position = 0

[[sound]]
hotkey = 'CTRL-P'
name = 'Nicht so tief, Rüdiger!'
path = 'nicht-so-tief-rudiger.mp3'

[[sound]]
name = 'Wer wird Millionär - Showstart'
path = 'https://www.myinstants.com//media/sounds/wer-wird-millionar-soundtracks-soundstart.mp3'
````

---

# Architecture overview

- sound <--> (mpsc channel)
  - http-api <--> webapp (js/html)
  - native-gui
  - telegram bot
- config <--> (rwlock)
  - http-api <--> webapp (js/html)
  - native-gui
  - telegram bot
- hotkey <--> (mpsc channel)
  - http-api <--> webapp (js/html)
  - native-gui

---

# Problems

- cross-plattform audio support
- cross-plattform hotkey support
- native gui: slow iteration and missing features
- Multithreading: config files and crates

---

##### problems

# cross-platform audio support

- no support for virtual microphones on any platform without additional drivers (windows, macos) or plugins (linux with pulseaudio)
  -> initial external user setup needed
- First version with rust audio crates cpal and rodio but only supports alsa on linux. Also few updates.
  -> Switched to miniaudio: C-Library with rust-binding but maintained.
- But miniaudio loopback stream didn't work on one arch linux dev system so we implemented native pulseaudio routing as a fallback on linux.

---

##### problems

# cross-platform hotkey support

- crate `hotkey-rs` supported windows and linux hotkeys
- added macos hotkey support via own c bindings
- also needed to rewrite `hotkey-rs`to support multi-thread usage and deregister functionality

---

##### problems

# native gui: slow iteration and missing features

- chose crate `iced` as our gui framework
  -> great cross-platform support, many features for such a young project
- not optimized for performance right now and no dynamic list support
  -> big soundboards with many buttons hard to implement
- But main problem with a native gui in rust:
  -> slow development iteration: compile times are a problem

---
<!-- _header: "" -->
<!-- _footer: "" -->

- current deps expand to 386 crates with default features
- incremental builds still take 10-30 seconds

```
anyhow = "1.0"
bytes = "0.5"
clap = "3.0.0-beta.1"
crossbeam-channel = "0.4"
dirs = "3"
env_logger = "0.7"
hotkey-soundboard = {path = "extern/hotkey-rs", version = "0.0.1"}
log = "0.4"
miniaudio = "0.7"
once_cell = "1.4"
parking_lot = "0.11"
regex = "1"
reqwest = {version = "0.10", features = ["blocking"]}
serde = {version = "1.0", features = ["derive"]}
strum = "0.18"
strum_macros = "0.18"
tokio = {version = "0.2", features = ["macros", "full", "sync", "time"]}
toml = "0.5"
winit = "0.22"

iced = {version = "0.1", optional = true, features = ["tokio"]}
iced_native = {version = "0.2", optional = true}

futures = {version = "0.3", optional = true}
warp = {version = "0.2", optional = true}

claxon = {version = "0.4", optional = true}
hound = {version = "3", optional = true}
lewton = {version = "0.10", optional = true}
minimp3 = {version = "0.3", optional = true}
mp3-duration = {version = "0.1.10", optional = true}
ogg_metadata = {version = "0.4", optional = true}
```

---

##### problems

# Multithreading

- five threads: http/webui, native gui, sound, hotkey and telegram bot
- central config file management with RwLock<Arc<Config>>. Fast reads but slow writes
  -> sqlite maybe better suited
- cross-thread communication via channels
- we use parking_lot mutexes because std library doesn't guarantee eventual fair locking
- no async usage but http and telegram crates use async
  -> multiple tokio runtimes

---

# Lessons learned

- Switched to webui: fast dev iteration, also possible to use your smartphone as controller
- Check your crates/dependencies thoroughly: last updates and issues, code quality etc.
- Crates like crossbeam or parking_lot are useful drop-in replacements for the standard library
- Github CI is really helpful and free for open-source projects. Cross-platform without continuous builds would be a nightmare.
- Multithreading/async is still hard also in Rust.

---

# Questions?

Try it out or contribute via https://github.com/gamebooster/soundboard

`cargo install soundboard` is already possible!
