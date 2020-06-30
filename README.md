# soundboard

[![Build](https://github.com/gamebooster/soundboard/workflows/Build/badge.svg)](https://github.com/gamebooster/soundboard/actions?query=workflow%3ABuild)

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

- play local and remote sounds (http) to your microphone and output device
  - supported codecs: mp3 (rfm: mp3), flac (rfm: flac), wav (rfm: wav), vorbis (rfm: vorbis), xm (rfm: xm, non-default)
- hotkeys
- native user interface (rfm: gui)
  - First iteration. The web user interface is slicker and performs better.
- web user interface and http api (rfm: http)
- telegram bot (rfm: telegram, non-default)
- automatic handling of loopback device in pulse audio (rfm: auto-loop, non-default)

### install

1. use compiled release package from https://github.com/gamebooster/soundboard/releases/
   or `cargo install soundboard` (compile time is a coffee break)
2. create soundboard config directory with soundboards (see below for example config)
3. provide virtual microphone (instructions below)
4. (optional) copy `web` directory in soundboard config directory for webui

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

stop_hotkey = "ALT-S" # stop all sound
http_server = true # api and webui; 3030 is the default port
no_gui = false # no native gui
```

</details>

<details>
  <summary>soundboards/favorites.toml</summary>

```
name = 'favorites'
position = 0

[[sound]]
name = 'Nicht so tief, Rüdiger!'
path = 'nicht-so-tief-rudiger.mp3'
hotkey = 'CTRL-P'
```

</details>

<details>
  <summary>soundboards/myinstants_soundboard.toml</summary>

```
name = "Myinstants.com"

[[sound]]
name="Sad Trombone"
path="https://www.myinstants.com//media/sounds/sadtrombone.swf.mp3"

[[sound]]
name="Dramatic Chipmunk"
path="https://www.myinstants.com//media/sounds/dramatic.swf.mp3"
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
