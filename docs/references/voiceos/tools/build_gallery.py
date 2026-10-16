import html,json,pathlib,subprocess
root=pathlib.Path(__file__).resolve().parents[1] / "visual-reference"
items=json.loads((root/"download-manifest.json").read_text())
downloaded=[i for i in items if i["status"]=="downloaded"]
labels={"screenshots":"Интерфейс","videos":"Видео","frames":"Кадры","images":"Материалы сайта","icons":"Иконки"}
esc=lambda x:html.escape(str(x),quote=True)
cards=[]
for item in sorted(downloaded,key=lambda i:(["screenshots","videos","images","icons"].index(i["category"]),i["file"])):
    cat=item["category"]
    alt=next((r.get("alt") for r in item["references"] if r.get("alt")),"")
    name=pathlib.Path(item["file"]).name.split("-",1)[-1]
    title=alt or name
    if cat=="videos":
        metadata=json.loads(subprocess.check_output(["ffprobe","-v","error","-show_entries","format=duration","-of","json",str(root/item["file"])]))
        duration=round(float(metadata["format"]["duration"]))
        content='<video controls preload="none" src="'+esc(item["file"])+'"></video>'
        note=str(duration)+" с · оригинальное видео"
    else:
        content='<a href="'+esc(item["file"])+'" target="_blank"><img loading="lazy" src="'+esc(item["file"])+'" alt="'+esc(title)+'"></a>'
        note=labels[cat]
    source='<a class="source" target="_blank" rel="noopener" href="'+esc(item["url"])+'">Источник ↗</a>'
    cards.append('<figure data-category="'+cat+'" data-search="'+esc((title+" "+name).lower())+'">'+content+'<figcaption><span class="tag">'+esc(note)+'</span><strong>'+esc(title)+'</strong><div class="links"><a target="_blank" href="'+esc(item["file"])+'">Открыть файл</a>'+source+'</div></figcaption></figure>')
for file in sorted((root/"video-frames").glob("*")):
    title=file.stem.replace("-"," ")
    rel=str(file.relative_to(root))
    cards.append('<figure data-category="frames" data-search="'+esc(title.lower())+'"><a href="'+esc(rel)+'" target="_blank"><img loading="lazy" src="'+esc(rel)+'" alt="'+esc(title)+'"></a><figcaption><span class="tag">Извлечено из официального видео</span><strong>'+esc(title)+'</strong></figcaption></figure>')
for file in sorted(root.glob("*website*.png")):
    title=file.stem.replace("-"," ")
    cards.append('<figure data-category="images" data-search="'+esc(title)+'"><a href="'+esc(file.name)+'" target="_blank"><img loading="lazy" src="'+esc(file.name)+'" alt="'+esc(title)+'"></a><figcaption><span class="tag">Снимок публичного сайта · статическое воспроизведение</span><strong>'+esc(title)+'</strong></figcaption></figure>')
buttons='<button class="active" data-filter="all">Все</button>'+''.join('<button data-filter="'+k+'">'+v+'</button>' for k,v in labels.items())
page='''<!doctype html><html lang="ru"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>VoiceOS — визуальные референсы</title>
<style>
:root{color-scheme:light;font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;background:#f4f5f7;color:#202124}
*{box-sizing:border-box}body{margin:0}header{padding:42px max(24px,5vw) 20px;background:#fff;border-bottom:1px solid #e6e8ec}h1{font-size:30px;letter-spacing:-1px;margin:0 0 12px}p{line-height:1.6;color:#596273;max-width:900px}a{color:#2563a6;text-decoration:none}a:hover{text-decoration:underline}.toolbar{position:sticky;top:0;z-index:2;background:#f4f5f7f0;backdrop-filter:blur(12px);padding:16px max(24px,5vw);display:flex;gap:9px;flex-wrap:wrap;border-bottom:1px solid #e5e7eb}button,input{font:inherit;border:1px solid #dce0e6;border-radius:20px;background:#fff;padding:9px 15px}button{cursor:pointer}button.active{color:#fff;background:#202124;border-color:#202124}input{min-width:240px;margin-left:auto}.counter{padding:12px max(24px,5vw) 0;font-size:13px;color:#687386}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:20px;padding:20px max(24px,5vw) 60px}figure{margin:0;overflow:hidden;border:1px solid #e1e5ea;border-radius:18px;background:#fff;box-shadow:0 5px 20px #18223507}figure>a{display:block;background:#e9edf2}figure img,figure video{display:block;width:100%;height:230px;object-fit:contain;background:#e9edf2}figure video{background:#111}figcaption{padding:17px;font-size:13px;line-height:1.5}strong{display:block;overflow-wrap:anywhere;font-weight:550;margin-top:8px}.tag{font-size:11px;color:#697486}.links{display:flex;justify-content:space-between;margin-top:13px;font-size:12px}footer{padding:0 max(24px,5vw) 36px;color:#737d8c;font-size:13px}[hidden]{display:none!important}@media(max-width:600px){input{margin-left:0;width:100%}.grid{grid-template-columns:1fr}}
</style>
<header><h1>VoiceOS — визуальные референсы</h1><p>5 оригинальных видео · 14 изображений с интерфейсом · 44 вспомогательных изображения · 53 иконки. Дополнительно — кадры видео и снимки сайта. Сбор публичных материалов для проектирования SpeakCursor.</p><p><a href="VISUAL_STYLE_RU.md">Разбор визуального стиля</a> · <a href="download-manifest.json">Источники всех файлов</a> · <a href="documentation-image-links.json">Дополнительные изображения документации — ссылки</a></p></header>
<div class="toolbar">'''+buttons+'''<input id="search" placeholder="Поиск по названию или описанию" aria-label="Поиск"></div><div id="counter" class="counter"></div><main class="grid">'''+''.join(cards)+'''</main><footer>Преимущественно показана macOS. Маркетинговые компоненты и изображения не подтверждают точное поведение установленного приложения. Изображения отдельного CDN документации сохранены ссылками: пробная загрузка была недоступна.</footer>
<script>
let category="all";const figures=[...document.querySelectorAll("figure")];const search=document.getElementById("search");
function update(){const q=search.value.toLowerCase();let visible=0;for(const f of figures){f.hidden=!((category==="all"||f.dataset.category===category)&&f.dataset.search.includes(q));if(!f.hidden)visible++}document.getElementById("counter").textContent="Показано: "+visible+" из "+figures.length}
for(const b of document.querySelectorAll("[data-filter]"))b.addEventListener("click",()=>{category=b.dataset.filter;for(const x of document.querySelectorAll("[data-filter]"))x.classList.toggle("active",x===b);update()});search.addEventListener("input",update);update();
</script></html>'''
(root/"gallery.html").write_text(page)
print("Gallery cards:",len(cards))
