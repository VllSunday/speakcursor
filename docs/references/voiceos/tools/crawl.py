import concurrent.futures, hashlib, json, pathlib, re, sys, urllib.request
from html.parser import HTMLParser

ROOT = pathlib.Path(__file__).resolve().parents[1] / "sources"
class Extract(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.hidden = 0
        self.out = []
        self.links = []
    def handle_starttag(self, tag, attrs):
        d=dict(attrs)
        if tag in ("script", "style", "svg", "noscript"):
            self.hidden += 1
        if tag in ("a", "link") and d.get("href"):
            self.links.append(d["href"])
        if self.hidden == 0 and tag in ("p","div","main","section","article","br","li","h1","h2","h3","h4","tr","td","dt","dd","details","summary"):
            self.out.append("\n")
    def handle_endtag(self, tag):
        if tag in ("script", "style", "svg", "noscript"):
            self.hidden=max(0,self.hidden-1)
        if self.hidden == 0 and tag in ("p","div","li","h1","h2","h3","h4","tr","dt","dd","summary"):
            self.out.append("\n")
    def handle_data(self, data):
        if not self.hidden:
            self.out.append(data)
def fetch(url):
    filename = re.sub(r"[^a-zA-Z0-9_-]", "_", url.removeprefix("https://"))[:180]
    try:
        req=urllib.request.Request(url,headers={"User-Agent":"Mozilla/5.0 (compatible; research)"})
        with urllib.request.urlopen(req,timeout=30) as r:
            data=r.read()
            kind=r.headers.get("content-type", "")
            final=r.url
        (ROOT/(filename+".raw")).write_bytes(data)
        txt=data.decode(errors="replace")
        links=[]
        if "html" in kind or txt.startswith("<!DOCTYPE"):
            p=Extract()
            p.feed(txt)
            txt="\n".join(line.strip() for line in "".join(p.out).splitlines() if line.strip())
            links=sorted(set(p.links))
        (ROOT/(filename+".txt")).write_text(txt)
        (ROOT/(filename+".links.json")).write_text(json.dumps(links,ensure_ascii=False,indent=2))
        return {"url":url,"final":final,"status":200,"bytes":len(data),"text_chars":len(txt),"file":filename+".txt"}
    except Exception as e:
        return {"url":url,"error":type(e).__name__+": "+str(e)}
if __name__=="__main__":
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        for result in pool.map(fetch,sys.argv[1:]):
            print(json.dumps(result,ensure_ascii=False))
