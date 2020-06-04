# soundboard

![](https://i.imgur.com/5OBElu2.png)


### config file format

soundboard.toml
````
# input_device = 0 # optional
# output_device = 1 # optional
# loopback_device = 2 # optional

[[sounds]] 
name = "Nicht so tief Rüdiger!" # display name
path = "nicht-so-tief-rudiger.mp3" # relative from sounds directory from exe path, formats: mp3, wav, flac
hotkey_modifier = ["CTRL"] # CTRL, SHIFT, SUPER, ALT possible
hotkey_key = "P" # numbers are KEY_9, special keys: BACKSPACE etc

[[sounds]] # how many you like
name = "vodka_dance"
path = "vodka/vodka_dance.mp3"
hotkey_modifier = ["CTRL", "SHIFT"]
hotkey_key = "P"
````

expected directory structure
````
soundboard{.exe}
soundboard.toml
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
1. write to `/etc/asound.conf`:
   ```
    pcm.pulseDeviceVirtualSink {
     type pulse
     device "virtualSink"
    }

   ``` 
2. enter command `pactl load-module module-null-sink sink_name=virtualSink`
3. use soundboard with loopback **virtual sink**
4. use applications with input *Monitor of Null Sink*

## default usage

1. run `soundboard --print-possible-devices`
2. run `soundboard --loopback-device <index>` 
    * loopback-device should be the installed virtual output device 
3. Press hotkeys or use gui to play sounds
4. `???`
5. Press `CTRL-C` to exit or press x on window