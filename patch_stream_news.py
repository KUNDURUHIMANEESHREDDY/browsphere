from pathlib import Path
p=Path(r"C:\Users\himan\Downloads\Browsphere\ui\public\index.html")
s=p.read_text(encoding='utf-8')
def replace_once(old,new):
 global s
 assert s.count(old)==1, (old[:90],s.count(old))
 s=s.replace(old,new,1)
replace_once("      var newTabNewsLastFetch = 0;", """      var newTabNewsLastFetch = 0;
      var newTabNewsStream = null;
      var newTabNewsStreamReady = false;
      var newTabNewsTimer = null;
      var newTabNewsDebounce = null;
      var newTabNewsCache = new Map();
      var newTabNewsIds = '';""")
replace_once("if (newTabNewsLoading || (!force && Date.now() - newTabNewsLastFetch < 300000)) return;", "if (newTabNewsLoading || (!force && Date.now() - newTabNewsLastFetch < 12000)) return;")
replace_once("""            var stories = await Promise.all(ids.slice(0, 35).map(async function (id) {
              try {
                var response = await fetch(base + 'item/' + id + '.json', { signal: controller.signal });
                return response.ok ? await response.json() : null;
              } catch (_) { return null; }
            }));""", """            var incomingIds = ids.slice(0, 35);
            var signature = incomingIds.join(',');
            if (signature === newTabNewsIds && newTabNewsItems.length) {
              newTabNewsLastFetch = Date.now();
              return;
            }
            var stories = await Promise.all(incomingIds.map(async function (id) {
              if (newTabNewsCache.has(id)) return newTabNewsCache.get(id);
              try {
                var response = await fetch(base + 'item/' + id + '.json', { signal: controller.signal });
                var story = response.ok ? await response.json() : null;
                if (story) newTabNewsCache.set(id, story);
                return story;
              } catch (_) { return null; }
            }));
            newTabNewsIds = signature;
            // Bound local cache to the stories currently shown.
            incomingIds.forEach(function (id) { /* preserve latest items */ });
            if (newTabNewsCache.size > 120) {
              Array.from(newTabNewsCache.keys()).slice(0, 80).forEach(function (id) { newTabNewsCache.delete(id); });
            }""")
replace_once("""          newTabNewsItems = rows;
          newTabNewsLastFetch = Date.now();""","""          newTabNewsItems = rows;
          newTabNewsLastFetch = Date.now();""")
replace_once("""          newTabNewsLoading = false;
          refreshNewsSection();
        }
      }

      function setupScrollProgress()""","""          newTabNewsLoading = false;
          refreshNewsSection();
        }
      }

      function stopLiveNewsStream() {
        if (newTabNewsStream) newTabNewsStream.close();
        newTabNewsStream = null;
        newTabNewsStreamReady = false;
        if (newTabNewsTimer) clearInterval(newTabNewsTimer);
        if (newTabNewsDebounce) clearTimeout(newTabNewsDebounce);
        newTabNewsTimer = null;
        newTabNewsDebounce = null;
      }

      function scheduleLiveNewsRefresh() {
        if (newTabNewsDebounce) clearTimeout(newTabNewsDebounce);
        newTabNewsDebounce = setTimeout(function () {
          newTabNewsDebounce = null;
          if (document.hidden || !document.getElementById('newTabNewsSection')) return;
          fetchLiveNews(true);
        }, 1500);
      }

      function startLiveNewsStream() {
        if (document.hidden || !document.getElementById('newTabNewsSection')) return;
        if (newTabNewsStream || newTabNewsTimer) return;
        fetchLiveNews(false);
        // Firebase REST streaming delivers changes as 'put' and 'patch' SSE events.
        // If native streaming is unavailable, a 15s fallback poll keeps news fresh.
        if (typeof EventSource !== 'undefined') {
          try {
            var stream = new EventSource('https://hacker-news.firebaseio.com/v0/newstories.json');
            newTabNewsStream = stream;
            stream.addEventListener('put', function () {
              newTabNewsStreamReady = true;
              scheduleLiveNewsRefresh();
            });
            stream.addEventListener('patch', function () {
              newTabNewsStreamReady = true;
              scheduleLiveNewsRefresh();
            });
            stream.onerror = function () {
              if (newTabNewsStream === stream) {
                stream.close();
                newTabNewsStream = null;
                newTabNewsStreamReady = false;
              }
            };
          } catch (_) { newTabNewsStream = null; }
        }
        newTabNewsTimer = setInterval(function () {
          if (document.hidden || !document.getElementById('newTabNewsSection')) {
            stopLiveNewsStream();
            return;
          }
          // Fallback when SSE cannot connect, with a slow safety refresh while connected.
          if (!newTabNewsStreamReady || Date.now() - newTabNewsLastFetch >= 60000) {
            fetchLiveNews(true);
          }
          if (!newTabNewsStream) {
            // Retry streaming on the next active interval without restarting the poll timer.
            if (typeof EventSource !== 'undefined') {
              stopLiveNewsStream();
              startLiveNewsStream();
            }
          }
        }, 15000);
      }

      function setupScrollProgress()""")
replace_once("if (document.getElementById('newTabNewsSection') && canvas.scrollTop > 80) fetchLiveNews(false);","if (document.getElementById('newTabNewsSection') && canvas.scrollTop > 80) startLiveNewsStream();")
replace_once("if (ns) { ns.scrollIntoView({ behavior: 'smooth', block: 'start' }); fetchLiveNews(false); }","if (ns) { ns.scrollIntoView({ behavior: 'smooth', block: 'start' }); startLiveNewsStream(); }")
replace_once("              fetchLiveNews(false);\n              renderActiveTab();", "              fetchLiveNews(false);\n              renderActiveTab();") if False else None
replace_once("""        setupScrollProgress();
        setupKeyboardShortcuts();""", """        setupScrollProgress();
        document.addEventListener('visibilitychange', function () {
          if (document.hidden) stopLiveNewsStream();
          else {
            var canvas = document.getElementById('readingCanvas');
            if (canvas && canvas.scrollTop > 80 && document.getElementById('newTabNewsSection')) startLiveNewsStream();
          }
        });
        setupKeyboardShortcuts();""")
p.write_text(s, encoding='utf-8')
print('Live SSE, polling fallback, cache and visibility handling installed')
