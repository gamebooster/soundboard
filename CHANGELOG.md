# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [0.1.0] - 2020-08-04

### Added

- **webui** has a lot of new features:
  - drag-drop support for files and links
  - copy-paste support for files and links
  - you can drag-drop all buttons at the same soundboard and to other soundboards
  - you can edit and delete sound buttons with a right click
- You can specify `start` and `end` timestamps (in seconds) for a sound [#8]
- Added `spotify` as sound source. [#7]

  - expected config format example: `source = {spotify = {id = "5y788ya4NvwhBznoDIcXwK"}}`
  - you need to provide `spotify-user` and `spotify-pass` via config, args or env

- Added `youtube` as sound source. [#6]

  - expected config format example: `source = {youtube = {id = "YUKeiJ3igXg"}}`
  - you need to provide `youtube-dl` and `mkvextract` in the `PATH`

- Added `tts` (text-to-speech) as sound source.

  - expected config format example: `source = {tts = {ssml = "<speak>Hello World!</speak>", lang = "en-GB"}}`
  - uses Google Cloud Text-to-Speech API

### Changed
- Switched to json as config format but toml is still possible as input format
  - toml crate doesn't support enum serialization
- Switched from `path` string to `source` enum for sounds in the config format to make the multiple possible sources clearer. Please see the `favorites.toml` for all possibilities and use the `converter_new_format.py` to convert your old soundboards
- backend: refactored config module into app_config and soundboards [#32][#31]

## [0.0.2] - 2020-06-30

### Added

- First release! :tada:
