# soundboard

### works

* on windows with sound and microphone playing on the same time 

### not working

* gui
* config
* code mess
* linux
* mac

### current usage

1. ` soundboard --print-possible-devices`
2. `soundboard -i <index> -o <index> -l <index>`
3. `Press CTRL-P to play nicht-so-tief-ruediger.mp3`
5. `???`
4. `Press CTRL-C to exit`

### providing virtual sink on linux 
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



#### example

````
PS K:\projects\soundboard> ./target/debug/soundboard -h
soundboard 0.1.0
Karl Skomski <karl@skomski.com>:Corin Baurmann <corin.baurmann@fu-berlin.de>:Elena Frank <elena.frank@protonmail.com>
play sounds over your microphone

USAGE:
    soundboard [FLAGS] [OPTIONS]

FLAGS:
    -h, --help                      Prints help information
        --no-gui                    Disable GUI
        --print-possible-devices    Print possible devices
    -V, --version                   Prints version information

OPTIONS:
    -c, --config <FILE>                        sets a custom config file [default: soundboard.toml]
    -i, --input-device <input-device>          Sets the input device to use
    -l, --loopback-device <loopback-device>    Sets the loopback device to use
    -o, --output-device <output-device>        Sets the output device to use
        --verbose <verbose>                    Sets the level of verbosity

PS K:\projects\soundboard> .\target\debug\soundboard.exe --print-possible-devices
  Devices:
  0. "CABLE Input (VB-Audio Virtual Cable)"
    Default output stream format:
      Format { channels: 2, sample_rate: SampleRate(48000), data_type: F32 }
  1. "Digital Audio (S/PDIF) (High Definition Audio Device)"
    Default output stream format:
      Format { channels: 2, sample_rate: SampleRate(48000), data_type: F32 }
  2. "Desktop Microphone (RØDE NT-USB Mini)"
    Default input stream format:
      Format { channels: 1, sample_rate: SampleRate(48000), data_type: F32 }
  3. "CABLE Output (VB-Audio Virtual Cable)"
    Default input stream format:
      Format { channels: 2, sample_rate: SampleRate(44100), data_type: F32 }
  4. "Microphone (HD Webcam C525)"
    Default input stream format:
      Format { channels: 1, sample_rate: SampleRate(48000), data_type: F32 }

PS K:\projects\soundboard> .\target\debug\soundboard.exe -i 2 -o 0 --no-gui
  Using Devices:
  2. "Desktop Microphone (RØDE NT-USB Mini)"
    Default input stream format:
      Format { channels: 1, sample_rate: SampleRate(48000), data_type: F32 }
  0. "CABLE Input (VB-Audio Virtual Cable)"
    Default output stream format:
      Format { channels: 2, sample_rate: SampleRate(48000), data_type: F32 }
Attempting to build input stream with `Format { channels: 1, sample_rate: SampleRate(48000), data_type: F32 }`.
Successfully built input stream.
Playing sound: K:\projects\soundboard\resources/nicht-so-tief-rudiger.mp3

Press CTRL-C to exit
````