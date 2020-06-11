#!/usr/bin/env python3
# -*- coding: utf-8 -*-


import requests
import bs4
from collections import Counter
import re

sounds = []

def get_download_link(name, sound_url):
    r = requests.get(sound_url)
    if r.status_code == 200:
        soup = bs4.BeautifulSoup(r.content, "html.parser")
        for sound in soup.find_all("a", {"class":"waves-effect waves-light btn blue white-text","download": True}):
            stripped = sound['href'].strip()
            sounds.append((name, f"https://www.myinstants.com/{stripped}"))
            print(name)


for index in range(1, 5):
    r = requests.get(
            f"https://www.myinstants.com/index/de/?page={index}")
    if r.status_code == 200:
            soup = bs4.BeautifulSoup(r.content, "html.parser")
            for sound in soup.find_all("a", {"class": "instant-link"}):
                get_download_link(sound.text.strip(), f"https://www.myinstants.com{sound['href'].strip()}")


with open("myinstants_soundboard.toml", "w") as text_file:
    for sound in sounds:
        print("[[sound]]", file=text_file)
        print(f"name=\"{sound[0]}\"", file=text_file)
        print(f"path=\"{sound[1]}\"", file=text_file)
        print("", file=text_file)