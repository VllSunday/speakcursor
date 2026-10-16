import concurrent.futures, hashlib, html, json, pathlib, re, urllib.parse, urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[1] / "sources"
OUT=ROOT.parent/"visual-reference"
inventory=json.loads((OUT/"media-inventory.json").read_text())
# CSS backgrounds and local media referenced by the core product pages.
for name in ["www_voiceos_com_.raw","www_voiceos_com_features.raw","www_voiceos_com_features_dictation.raw","www_voiceos_com_features_agent.raw"]:
    text=(ROOT/name).read_text()
    for ref in re.findall(r"url\\?\\?\(([^)]+)\)",text):
        ref=html.unescape(ref.strip(" ' \""))
        if ref.startswith("/") and re.search(r"\.(?:webp|png|jpg|svg)$",ref):
            inventory.setdefault("https://www.voiceos.com"+ref,[{"page":name,"type":"background"}])
    for ref in re.findall(r'/(?:[A-Za-z0-9_./-]+)\.(?:webp|png|jpe?g|gif|mp4|webm)', text):
        if not ref.startswith("//"):
            inventory.setdefault("https://www.voiceos.com"+ref,[{"page":name,"type":"source-reference"}])
def category(url):
    path=urllib.parse.urlparse(url).path
    if path.endswith((".mp4",".webm",".mov")): return "videos"
    if "/icons/" in path or path.endswith(".svg") or "Logo" in path: return "icons"
    if any(x in path for x in ["/guides/workflows/","/rewrite-edit-text-by-voice-mac-windows/","/posts/voiceos-email-workflow/","/agent-mode-send-message","/send-slack-message-by-voice-confirmation"]): return "screenshots"
    return "images"
def get(item):
    url,refs=item
    if urllib.parse.urlparse(url).hostname!="www.voiceos.com":
        return {"url":url,"status":"reference_only_external","references":refs}
    cat=category(url)
    name=pathlib.Path(urllib.parse.urlparse(url).path).name
    key=hashlib.sha256(url.encode()).hexdigest()[:8]
    target=OUT/cat/(key+"-"+name)
    target.parent.mkdir(exist_ok=True)
    try:
        request=urllib.request.Request(url,headers={"User-Agent":"Mozilla/5.0"})
        with urllib.request.urlopen(request,timeout=40) as r:
            size=int(r.headers.get("content-length","0"))
            if size>200_000_000:
                return {"url":url,"status":"reference_only_large","bytes":size,"references":refs}
            data=r.read(200_000_001)
        if len(data)>200_000_000: raise ValueError("Media exceeds 200 MB per-file budget")
        target.write_bytes(data)
        return {"url":url,"status":"downloaded","file":str(target.relative_to(OUT)),"bytes":len(data),"category":cat,"references":refs}
    except Exception as e:
        return {"url":url,"status":"unavailable","error":type(e).__name__+": "+str(e),"references":refs}
items=list(inventory.items())
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    results=[]
    for result in pool.map(get,items):
        results.append(result)
        print(json.dumps({k:v for k,v in result.items() if k!="references"},ensure_ascii=False),flush=True)
(OUT/"download-manifest.json").write_text(json.dumps(results,ensure_ascii=False,indent=2))
print("TOTAL",len(results),"DOWNLOADED",sum(x["status"]=="downloaded" for x in results),flush=True)
