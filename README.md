# soundboard

[![cratesio](https://img.shields.io/crates/v/soundboard.svg)](https://crates.io/crates/soundboard)
[![BuildDebug](https://github.com/gamebooster/soundboard/workflows/BuildDebug/badge.svg)](https://github.com/gamebooster/soundboard/actions?query=workflow%3ABuildDebug)
[![BuildRelease](https://github.com/gamebooster/soundboard/workflows/BuildRelease/badge.svg)](https://github.com/gamebooster/soundboard/actions?query=workflow%3ABuildRelease)

cross-platform desktop application to spice up your audio/video conferences

  <img alt="nativeui" src="https://i.imgur.com/5OBElu2.png"/>
  <figcaption>(screenshot is out of date but captures the essence.)</figcaption>

<details>
  <summary>More screenshots</summary>

<p float="left">
  <img alt="webui" src="https://i.imgur.com/4AD4DNp.png" width="55%" /> 
  <img alt="telegram" src="https://i.imgur.com/o9WByEN.jpg" width="44%" /><figcaption>Web UI and Telegram Bot</figcaption>
</p>

</details>

### features (rust feature name: rfm)

- play local and remote sounds to your microphone and output device
  - supported codecs
    - mp3 (rfm: mp3)
    - flac (rfm: flac)
    - wav (rfm: wav)
    - vorbis (rfm: vorbis)
    - opus (rfm: opus)
    - xm (rfm: xm, non-default)
  - supported sources (config example at the bottom):
    - local (files)
    - http
    - tts (rfm: text-to-speech)
    - spotify (rfm: spotify)
    - youtube
- global hotkeys
  - default `stop-hotkey` for all sounds is `CTRL-ALT-E`
- web user interface and http api (rfm: http)
  - default socket addr: `127.0.0.1:8080`
- text user interface (rfm: textui)
- native graphical user interface (rfm: gui, non-default)
  - First iteration. The web user interface is slicker and performs better.
- telegram bot (rfm: telegram, non-default)
  - you need to create a bot and then specify your `telegram-token`
- automatic handling of loopback device in pulse audio (rfm: auto-loop, non-default)

### config, env and command line options

- you can provide all options via the config file `soundboard.toml`, env variables `SB_<option>` or via the command line `--<option>`
  - example: `gui = true` or `SB_GUI=true` or `--gui=true`
- use `--help` to see all options

### install

1. use compiled release package from https://github.com/gamebooster/soundboard/releases/
   or `cargo install soundboard` (compile time is a coffee break)
2. create soundboard config directory with soundboards (see below for example config)
3. provide virtual microphone (instructions below)
4. (optional) add `youtube-dl` and `mkvextract` to PATH variable to use youtube as source
5. (optional) provide `spotify-user` and `spotify-pass` via args, config, or env to use spotify as source. You need a premium account.

## default usage

1. run `soundboard --print-possible-devices`
2. run `soundboard --loopback-device "<name>"` or put in config file
   - loopback-device should be the installed virtual output device name
3. Press hotkeys or use native gui or open web ui http://localhost:3030
4. `???`
5. Press `CTRL-C` to exit or press x on window

### providing virtual microphone on windows

1. download and install vb-audio virtual cable from https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip
2. start soundboard with loopback device `CABLE Input`
3. use applications with input `CABLE Output`

### providing virtual microphone on linux

1. create and choose loopback device  
   a. use flag --auto-loop-device  
   b. alternative: enter command `pactl load-module module-null-sink sink_name=virtualSink`
2. start soundboard with loopback device `null sink`
3. use applications with input `Monitor of Null Sink` or `Monitor of SoundboadLoopbackDevice`

### providing virtual microphone on macos

1. download and install soundflower kernel extension from https://github.com/mattingalls/Soundflower/releases
2. set sample rate via Audio MIDI Setup for Soundflower (2ch) to 48000 hz
3. start soundboard with loopback device: `Soundflower (2ch)`
4. use applications with input: `Soundflower (2ch)`

### config file example

soundboard.toml is optional. soundboards directory is mandatory.

config search path:

```
{soundboard exe location}
$XDG_CONFIG_HOME/soundboard/
$HOME/.config/soundboard/
$HOME/.soundboard/
```

<details>
  <summary>soundboard.toml</summary>

```
# input_device = "Mikrofonarray (Realtek High Definition Audio(SST))" # optional else default device
# output_device = "Speaker/HP (Realtek High Definition Audio(SST))" # optional else default device
loopback_device = "CABLE Input (VB-Audio Virtual Cable)" # required: change to your virtual loopback output

stop_hotkey = "CTRL-ALT-E" # stop all sound
```

</details>

<details>
  <summary>soundboards/favorites.toml</summary>

```
name = 'favorites'
position = 0 # always position ahead of other soundboards

[[sound]]
hotkey = 'CTRL-SHIFT-BACKSPACE'
name = 'Soldier of Fortune'
source = {local = {path = 'vodka/Razor1911 - Soldier Of Fortune intro.xm'}}

[[sound]]
name = 'steam incoming'
source = {http = {url = 'https://www.myinstants.com/media/sounds/message_2.mp3'}}

[[sound]]
hotkey = 'CTRL-P'
name = 'Nicht so tief, Rüdiger!'
source = {local = {path = 'nicht-so-tief-rudiger.mp3'}}

[[sound]]
end = 10.5 # end sound timestamp, supported for all sources
name = "Sound of Silence"
source = {spotify = {id = "5y788ya4NvwhBznoDIcXwK"}}
start = 2 # start sound timestamp, supported for all sources

[[sound]]
end = 18.5
name = "dreams"
source = {youtube = {id = "ZXsQAXx_ao0"}}
start = 14

[[sound]]
end = 58
name = "tired"
source = {youtube = {id = "ZXsQAXx_ao0"}}
start = 53

[[sound]]
name = '''Looks Like You're F'd'''
source = {http = {url = 'https://www.soundboard.com/handler/playTrack.ashx?id=893190', headers = [{name = 'referer', value = 'https://www.soundboard.com/'}]}}

[[sound]]
name = "Hello World"
source = {tts = {ssml = '''
<speak>
Hello World!
</speak>
''', lang = "en-GB"}}
```

</details>

<details>
  <summary>soundboards/myinstants_soundboard.toml</summary>

```
name = "Myinstants.com"

[[sound]]
name = 'Falcon Punch'
source = {http = {url = 'https://www.myinstants.com//media/sounds/falconpunch.swf.mp3'}}

[[sound]]
name = 'Knaller'
source = {http = {url = 'https://www.myinstants.com//media/sounds/videoplayback-2-online-audio-converter.mp3'}}
```

</details>

<details>
  <summary>expected directory structure for example config files</summary>

```
soundboard.toml
soundboards/
  favorites/
    nicht-so-tief-rudiger.mp3
  favorites.toml
  myinstants_soundboard.toml
```

</details>
