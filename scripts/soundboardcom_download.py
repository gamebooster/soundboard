#!/usr/bin/env python3
# -*- coding: utf-8 -*-


import requests
import bs4
from collections import Counter
import re

sounds = []

soundboard_name = "solrosin"

r = requests.get(
        f"https://www.soundboard.com/sb/{soundboard_name}")
if r.status_code == 200:
        soup = bs4.BeautifulSoup(r.content, "html.parser")
        for sound in soup.find_all("a", {"class": "track tracktitle jp-playlist-item"}):
            sounds.append((sound['title'].strip(), f"https://www.soundboard.com/handler/playTrack.ashx?id={sound['data-track-id'].strip()}"))

with open(f"{soundboard_name}_soundboard.toml", "w") as text_file:
    for sound in sounds:
        print("[[sound]]", file=text_file)
        print(f"name=\"{sound[0]}\"", file=text_file)
        print(f"path=\"{sound[1]}\"", file=text_file)
        print("  [[sound.header]]", file=text_file)
        print("    name=\"referer\"", file=text_file)
        print("    value=\"https://www.soundboard.com/\"", file=text_file)
        print("", file=text_file)