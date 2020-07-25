import tomlkit
import pathlib
import argparse
from argparse import ArgumentParser
import re

parser = ArgumentParser(description="convert soundboards to new format")
parser.add_argument("-i", dest="input", required=True,
                    help="old soundboard file path", metavar="FILE")
parser.add_argument("-o", dest="output", required=True,
                    help="new soundboard file path", metavar="FILE")
args = parser.parse_args()

soundboard = tomlkit.parse(pathlib.Path(
    args.input).read_text('utf-8'))

youtube_pattern = re.compile(
    "(?:https?:\/\/)?(?:www\.)?youtu\.?be(?:\.com)?\/?.*(?:watch|embed)?(?:.*v=|v\/|\/)([\w\-_]+)\&?")

for sound in soundboard["sound"]:
    sound["source"] = tomlkit.inline_table()
    if "<speak>" in sound["path"]:
        sound["source"]["tts"] = tomlkit.inline_table()
        sound["source"]["tts"]["ssml"] = sound["path"]
        sound["source"]["tts"]["lang"] = sound["tts_language"]
    elif "youtube.com" in sound["path"] or "youtu.be" in sound["path"]:
        sound["source"]["youtube"] = tomlkit.inline_table()
        sound["source"]["youtube"]["id"] = youtube_pattern.match(
            sound["path"]).group(1)
    elif sound["path"].startswith("http"):
        if "header" in sound:
            sound["source"]["http"] = tomlkit.inline_table()
            sound["source"]["http"]["url"] = sound["path"]
            header = tomlkit.array()
            for val in sound["header"]:
                inline = tomlkit.inline_table()
                inline.append("name", val["name"])
                inline.append("value", val["value"])
                header.append(inline)
            sound["source"]["http"]["headers"] = header
        else:
            sound["source"]["http"] = tomlkit.inline_table()
            sound["source"]["http"]["url"] = sound["path"]
    else:
        sound["source"]["local"] = tomlkit.inline_table()
        sound["source"]["local"]["path"] = sound["path"]

    del sound["path"]
    if "header" in sound:
        del sound["header"]
    if "tts_language" in sound:
        del sound["tts_language"]
    if "tts_options" in sound:
        del sound["tts_options"]

pathlib.Path(args.output).write_text(tomlkit.dumps(soundboard), 'utf-8')
