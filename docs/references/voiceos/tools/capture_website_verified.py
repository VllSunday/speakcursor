import hashlib,json,pathlib,urllib.parse,urllib.request
from playwright.sync_api import sync_playwright
root=pathlib.Path(__file__).resolve().parents[1] / "sources"
out=root.parent/"visual-reference"
cache=out/"website-source"/"resource-cache";cache.mkdir(exist_ok=True)
downloaded=json.loads((out/"download-manifest.json").read_text())
saved={x["url"]:out/x["file"] for x in downloaded if x["status"]=="downloaded"}
raw={"/":root/"www_voiceos_com_.raw","/features/dictation":root/"www_voiceos_com_features_dictation.raw","/features/agent":root/"www_voiceos_com_features_agent.raw"}
misses=[]
def serve(route):
    url=route.request.url
    parsed=urllib.parse.urlsplit(url)
    if parsed.hostname!="www.voiceos.com" or parsed.path.startswith(("/api/","/auth/","/ingest/")):
        route.abort();return
    if parsed.path.endswith((".mp4",".webm")):
        route.fulfill(status=204,body="");return
    try:
        if parsed.path in raw:
            data=raw[parsed.path].read_bytes();ctype="text/html; charset=utf-8"
        elif url in saved:
            data=saved[url].read_bytes()
            import mimetypes
            ctype=mimetypes.guess_type(parsed.path)[0] or "application/octet-stream"
        else:
            key=hashlib.sha256(url.encode()).hexdigest()
            file=cache/key
            metadata=cache/(key+".json")
            if file.exists() and metadata.exists():
                data=file.read_bytes();ctype=json.loads(metadata.read_text())["content_type"]
            else:
                request=urllib.request.Request(url,headers={"User-Agent":"Mozilla/5.0","Accept-Encoding":"identity"})
                with urllib.request.urlopen(request,timeout=12) as response:
                    data=response.read();ctype=response.headers.get("content-type","application/octet-stream")
                file.write_bytes(data);metadata.write_text(json.dumps({"url":url,"content_type":ctype}))
        route.fulfill(status=200,content_type=ctype,body=data)
    except Exception as e:
        misses.append({"path":parsed.path,"error":type(e).__name__})
        route.abort()
with sync_playwright() as p:
    browser=p.chromium.launch(executable_path="/usr/bin/chromium",headless=True,args=["--no-sandbox","--disable-dev-shm-usage"])
    page=browser.new_page(viewport={"width":1440,"height":1050},device_scale_factor=1)
    page.route("**/*",serve)
    observations=[]
    for path,name in [("/","home"),("/features/dictation","dictation"),("/features/agent","agent")]:
        try:
            page.goto("https://www.voiceos.com"+path,wait_until="domcontentloaded",timeout=45000)
            page.wait_for_timeout(1200)
            page.screenshot(path=str(out/(name+"-website.png")),full_page=True)
            observations.append({"name":name,"status":"captured_static_replay","title":page.title()})
            for selector,label in [(".ds-fast","dictation-speed"),(".dc-card","dictation-cleanup"),(".ia-wallpaper","agent-apps"),(".cf-panel","agent-background")]:
                loc=page.locator(selector).first
                if loc.count() and loc.is_visible():
                    loc.scroll_into_view_if_needed()
                    page.wait_for_timeout(2400)
                    loc.screenshot(path=str(out/(label+"-website-component.png")))
        except Exception as e:
            observations.append({"name":name,"status":"failed","error":str(e).splitlines()[0]})
    browser.close()
(out/"browser-observations.json").write_text(json.dumps({"mode":"offline replay of TLS-verified public page resources; demo videos omitted","pages":observations,"unavailable":misses},ensure_ascii=False,indent=2))
print(json.dumps(observations,ensure_ascii=False))
