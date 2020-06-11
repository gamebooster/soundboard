#!/usr/bin/env python3
# -*- coding: utf-8 -*-

from selenium import webdriver
from selenium import common
from selenium.webdriver.firefox.options import Options
import bs4
from collections import Counter
import re
import requests
import shutil
from pathlib import Path
import os
import concurrent.futures

target_url = "https://www.101soundboards.com/boards/28680-deutsche-memes-german-mlg-meme-soundboard-deu-de"
target_name = "deutsche-memes_soundboard"
download_dir = "K:\\projects\\soundboard\\scripts\\deutsche-memes_soundboard"
target_name_html = f"{target_name}.html"
target_name_toml = f"{target_name}.toml"
sounds = []


def download_file(local_path, url):
    options = Options()
    options.headless = True
    p = webdriver.FirefoxProfile()
    p.set_preference("browser.download.folderList", 2)
    p.set_preference("browser.download.manager.showWhenStarting", False)
    p.set_preference("browser.download.dir", download_dir)
    p.set_preference("browser.helperApps.neverAsk.saveToDisk", "audio/mpeg")
    p.set_preference("media.play-stand-alone", False)
    driver = webdriver.Firefox(options=options,firefox_profile=p)
    driver.set_page_load_timeout(5)
    try:
        driver.get(url)
    except common.exceptions.TimeoutException:
        pass
    driver.close()
    file_name = (url.split('/')[-1]).split('?')[0]
    os.rename(f"{target_name}/{file_name}", local_path)

html_file = Path(target_name_html)
page_source = None
if html_file.is_file() is False:
    driver = webdriver.Firefox()
    driver.get(target_url)
    page_source = driver.page_source
    with open(target_name_html, "w", encoding="utf-8") as html_file:
        html_file.write(driver.page_source)
else:
  with open(target_name_html, 'r', encoding="utf-8") as html_file:
    page_source = html_file.read()
  
soup = bs4.BeautifulSoup(page_source, "html.parser")

for sound in soup.find_all("div", {"class": "soundPlayer_text"}):
    name = sound.text.strip()
    download_link = f"https://www.101soundboards.com/{sound.parent.parent.find_all('source')[0]['src']}"
    sounds.append((name, download_link))

with open(target_name_toml, "w", encoding="utf-8") as text_file:
    if os.path.exists(target_name) is False:
      os.mkdir(target_name)
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        for sound in sounds:
            local_path = f"{target_name}/{sound[0]}.mp3"
            executor.submit(download_file, local_path, sound[1])
            print("[[sound]]", file=text_file)
            print(f"name=\"{sound[0]}\"", file=text_file)
            print(f"path=\"{local_path}\" # {sound[1]}", file=text_file)
            print("", file=text_file)