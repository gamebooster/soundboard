# soundboard

![Build](https://github.com/gamebooster/soundboard/workflows/Build/badge.svg)

cross-platform desktop application to spice up your audio/video conferences


![](https://i.imgur.com/5OBElu2.png)


### config file format

soundboard.toml
````
# input_device = "Mikrofonarray (Realtek High Definition Audio(SST))"
# output_device = "Speaker/HP (Realtek High Definition Audio(SST))"
loopback_device = "CABLE Input (VB-Audio Virtual Cable)" # change to your virtual loopback output

stop_hotkey = "ALT-S"
http_server = true # 3030 is the default port
no_gui = false

[[soundboard]]
name = "favorites" # display name for soundboard

    [[soundboard.sound]] # array of sounds
    name = "Nicht so tief, Rüdiger!" # display name
    path = "sounds/nicht-so-tief-rudiger.mp3" # relative from sounds directory from exe path, formats: mp3, wav, flac, ogg
    hotkey = "CTRL-P" # optional hotkey CTRL,SHIFT,SUPER,ALT possible

    [[soundboard.sound]]
    name = "Razor1911 Vodka Dance"
    path = "sounds/vodka/vodka_dance.mp3"
    hotkey = "CTRL-SHIFT-BACKSPACE"

    [[soundboard.sound]]
    name = "It's time to duel"
    path = "sounds/its-time-to-duel.ogg"
    hotkey = "ALT-9"
````

myinstants_soundboard.toml
````
name = "Myinstants.com"

[[sound]]
name="Sad Trombone"
path="https://www.myinstants.com//media/sounds/sadtrombone.swf.mp3"

[[sound]]
name="Dramatic Chipmunk"
path="https://www.myinstants.com//media/sounds/dramatic.swf.mp3"
````

expected directory structure for config file from above
````
soundboard{.exe}
soundboard.toml
soundboards/
  myinstants_soundboard.toml
sounds/
  nicht-so-tief-rudiger.mp3
  vodka/
    vodka_dance.mp3
````

### works

* on windows with sound and microphone playing on the same time 
* gui
* linux
* config

### not working

* code mess
* mac

### providing virtual microphone on windows

1. download and install vb-audio virtual cable from https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip
2. select `CABLE Output` as your microphone in your voice app like discord etc`

### providing virtual microphone on linux 
1. enter command `pactl load-module module-null-sink sink_name=virtualSink`
2. use soundboard with loopback **null sink**
3. use applications with input *Monitor of Null Sink*

## default usage

1. run `soundboard --print-possible-devices`
2. run `soundboard --loopback-device "<name>"` or put in config file
    * loopback-device should be the installed virtual output device 
3. Press hotkeys or use gui to play sounds or open web ui
4. `???`
5. Press `CTRL-C` to exit or press x on window
