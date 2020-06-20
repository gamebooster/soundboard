# soundboard

[![Build](https://github.com/gamebooster/soundboard/workflows/Build/badge.svg)](https://github.com/gamebooster/soundboard/actions?query=workflow%3ABuild)

cross-platform desktop application to spice up your audio/video conferences


![soundboard screenshot](https://i.imgur.com/5OBElu2.png)

### features

* play local and remote sounds (http) to your microphone and output device
* hotkeys
* native user interface
* web user interface
* http api

## default usage

1. run `soundboard --print-possible-devices`
2. run `soundboard --loopback-device "<name>"` or put in config file
    * loopback-device should be the installed virtual output device name
3. Press hotkeys or use gui to play sounds or open web ui
4. `???`
5. Press `CTRL-C` to exit or press x on window

### providing virtual microphone on windows

1. download and install vb-audio virtual cable from https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip
2. select `CABLE Output` as your microphone in your voice app like discord etc`

### providing virtual microphone on linux 
1. create and choose loopback device   
    a. use flag --auto-loop-device   
    b. alternative: enter command `pactl load-module module-null-sink sink_name=virtualSink`   
    and use soundboard with loopback **null sink**
3. use applications with input *Monitor of Null Sink* or *Monitor of SoundboadLoopbackDevice*

### config file example


<details>
  <summary>soundboard.toml</summary>

````
# input_device = "Mikrofonarray (Realtek High Definition Audio(SST))" # optional else default device
# output_device = "Speaker/HP (Realtek High Definition Audio(SST))" # optional else default device
loopback_device = "CABLE Input (VB-Audio Virtual Cable)" # required: change to your virtual loopback output

stop_hotkey = "ALT-S" # stop all sound
http_server = true # api and webui; 3030 is the default port
no_gui = false # no native gui
````
</details>


<details>
  <summary>soundboards/favorites.toml</summary>

````
name = 'favorites'
position = 0

[[sound]]
name = 'Nicht so tief, Rüdiger!'
path = 'nicht-so-tief-rudiger.mp3'
hotkey = 'CTRL-P'
````
</details>


<details>
  <summary>soundboards/myinstants_soundboard.toml</summary>

````
name = "Myinstants.com"

[[sound]]
name="Sad Trombone"
path="https://www.myinstants.com//media/sounds/sadtrombone.swf.mp3"

[[sound]]
name="Dramatic Chipmunk"
path="https://www.myinstants.com//media/sounds/dramatic.swf.mp3"
````
</details>

<details>
  <summary>expected directory structure for example config files</summary>

````
soundboard{.exe}
soundboard.toml
soundboards/
  favorites/
    nicht-so-tief-rudiger.mp3
  favorites.toml
  myinstants_soundboard.toml
````
</details>
