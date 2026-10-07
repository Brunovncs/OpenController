"""Downloads Wikimedia Commons files at a given width into a folder: fetch_refs.py OUT_DIR WIDTH name=File:Title ..."""
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

UA = {"User-Agent": "OpenControllerArtResearch/1.0 (https://github.com/Brunovncs/OpenController)"}


def get(url: str) -> bytes:
    import subprocess
    return subprocess.run(["curl", "-sfL", "-A", UA["User-Agent"], url], check=True, capture_output=True).stdout


out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
width = int(sys.argv[2])
for arg in sys.argv[3:]:
    name, title = arg.split("=", 1)
    q = urllib.parse.urlencode({"action": "query", "titles": title, "prop": "imageinfo", "iiprop": "url|size", "iiurlwidth": width, "format": "json"})
    pages = json.loads(get("https://commons.wikimedia.org/w/api.php?" + q))["query"]["pages"]
    info = next(iter(pages.values()))["imageinfo"][0]
    url = info.get("thumburl") or info["url"]
    ext = url.split("?")[0].rsplit(".", 1)[-1].lower()[:4]
    (out / f"{name}.{ext}").write_bytes(get(url))
    print(name, info["width"], "x", info["height"], "->", url[:90])
