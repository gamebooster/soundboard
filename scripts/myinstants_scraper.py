#!/usr/bin/env python3
# -*- coding: utf-8 -*-


import requests
import bs4
from collections import Counter
import re
import concurrent.futures
import multiprocessing

lock = multiprocessing.Lock()

text_file = open("myinstants_soundboard.toml", "w", encoding='utf-8')


def get_download_link(name, sound_url):
    r = requests.get(sound_url)
    if r.status_code == 200:
        soup = bs4.BeautifulSoup(r.content, "html.parser")
        for sound in soup.find_all("a", {"download": True}):
            stripped = sound['href'].strip()
            link = f"https://www.myinstants.com/{stripped}"
            with lock:
                print(name, link)
                print("[[sound]]", file=text_file)
                print(f"name=\"{name}\"", file=text_file)
                print(f"path=\"{link}\"", file=text_file)
                print("", file=text_file, flush=True)


with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
    for index in range(1, 200):
        print(f"at index: {index}")
        r = requests.get(
            f"https://www.myinstants.com/index/de/?page={index}")
        if r.status_code == 200:
            soup = bs4.BeautifulSoup(r.content, "html.parser")
            for sound in soup.find_all("a", {"class": "instant-link"}):
                name = sound.text.strip()
                link = f"https://www.myinstants.com{sound['href'].strip()}"
                executor.submit(get_download_link, name, link)
