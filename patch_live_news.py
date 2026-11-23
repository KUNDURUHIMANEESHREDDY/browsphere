from pathlib import Path
import re
p = Path(r"C:\Users\himan\Downloads\Browsphere\ui\public\index.html")
s = p.read_text(encoding="utf-8")
s,n = re.subn(r"      var newTabNewsItems = \[\n.*?\n      \];\n      var selectedTextCache", "      var newTabNewsItems = [];\n      var selectedTextCache", s, count=1, flags=re.S)
assert n == 1, "news fixture not found"
s = s.replace("var leadStory = filteredNews[0] || newTabNewsItems[0];", "var leadStory = filteredNews[0] || null;")
s = s.replace("var leadHtml = (\n              '<button type=\"button\"", "var leadHtml = leadStory ? (\n              '<button type=\"button\"",1)
s = s.replace("            var listHtml = restStories.map(function (st) {", "            var listHtml = restStories.map(function (st) {",1)
s = s.replace("              '</button>'\n            );\n\n            var listHtml = restStories", "              '</button>'\n            ) : '';\n\n            var listHtml = restStories",1)
s = s.replace("                      leadHtml +\n", "                      (leadHtml || '<p class=\"text-[13px] opacity-65 py-6\">' + (newTabNewsLoading ? 'Loading current headlines…' : 'Scroll here to load current headlines.') + '</p>') +\n",1)
s = s.replace("                      listHtml +\n", "                      (listHtml || (newTabNewsError ? '<p class=\"text-[13px] opacity-65 py-6\">' + esc(newTabNewsError) + '</p>' : '')) +\n",1)
s = s.replace("var newTabNewsCategory = 'All';", "var newTabNewsCategory = 'All';\n      var newTabNewsLoading = false;\n      var newTabNewsError = '';\n      var newTabNewsLastFetch = 0;",1)
# Reload just the news part to preserve scroll position and chat draft.
needle = "      function setupScrollProgress() {"
insert = r"""      // Uses the official Hacker News Firebase API: actual published stories, not mock headlines.
      // Public API is keyless and supports browser CORS.
      function newsCategory(title) {
        var t = title.toLowerCase();
        if (/ai\b|llm|model|machine learning|neural|openai|anthropic|inference/.test(t)) return 'AI & Systems';
        if (/design|ui\b|ux\b|typography|css\b|interface|visual/.test(t)) return 'Design';
        if (/science|research|space|physics|biology|climate|quantum/.test(t)) return 'Science';
        return 'Engineering';
      }

      function newsRelativeTime(ts) {
        var mins = Math.max(0, Math.floor((Date.now() / 1000 - ts) / 60));
        if (mins < 60) return (mins || 1) + 'm ago';
        if (mins < 1440) return Math.floor(mins / 60) + 'h ago';
        return Math.floor(mins / 1440) + 'd ago';
      }

      function refreshNewsSection() {
        var current = document.getElementById('newTabNewsSection');
        if (!current) return;
        var holder = document.createElement('div');
        holder.innerHTML = getPageData('browsphere://start').renderHtml();
        var updated = holder.querySelector('#newTabNewsSection');
        if (updated) current.innerHTML = updated.innerHTML;
      }

      async function fetchLiveNews(force) {
        if (newTabNewsLoading || (!force && Date.now() - newTabNewsLastFetch < 300000)) return;
        newTabNewsLoading = true;
        newTabNewsError = '';
        refreshNewsSection();
        try {
          var base = 'https://hacker-news.firebaseio.com/v0/';
          var controller = new AbortController();
          var timer = setTimeout(function () { controller.abort(); }, 12000);
          var ids;
          try {
            var idsResponse = await fetch(base + 'newstories.json', { signal: controller.signal });
            if (!idsResponse.ok) throw new Error('Headlines unavailable');
            ids = await idsResponse.json();
            if (!Array.isArray(ids)) throw new Error('Invalid headlines response');
            var stories = await Promise.all(ids.slice(0, 35).map(async function (id) {
              try {
                var response = await fetch(base + 'item/' + id + '.json', { signal: controller.signal });
                return response.ok ? await response.json() : null;
              } catch (_) { return null; }
            }));
          } finally {
            clearTimeout(timer);
          }
          var rows = stories.filter(function (st) {
            return st && st.type === 'story' && !st.deleted && !st.dead &&
              typeof st.title === 'string' && st.title.trim() &&
              typeof st.time === 'number';
          }).map(function (st) {
            var destination = typeof st.url === 'string' ? st.url : 'https://news.ycombinator.com/item?id=' + st.id;
            try {
              var parsed = new URL(destination);
              if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') return null;
            } catch (_) { return null; }
            return {
              title: st.title,
              category: newsCategory(st.title),
              source: 'Hacker News',
              dek: 'Read the original story and discussion from the live Hacker News feed.',
              url: destination,
              readTime: 'Latest story',
              timeAgo: newsRelativeTime(st.time),
              thumbSvg: '<svg viewBox="0 0 120 72" class="w-full h-full" aria-hidden="true"><rect width="120" height="72" rx="8" fill="#F4F6FB"/><text x="60" y="48" text-anchor="middle" font-size="44" font-family="Arial" fill="#FF6600">Y</text></svg>'
            };
          }).filter(Boolean);
          if (!rows.length) throw new Error('No new stories available');
          newTabNewsItems = rows;
          newTabNewsLastFetch = Date.now();
        } catch (e) {
          newTabNewsError = 'Live news could not load. Check your internet connection and scroll again to retry.';
        } finally {
          newTabNewsLoading = false;
          refreshNewsSection();
        }
      }

"""
assert needle in s
s=s.replace(needle,insert+needle,1)
s=s.replace("        canvas.addEventListener('scroll', function () {\n          var max = canvas.scrollHeight", "        canvas.addEventListener('scroll', function () {\n          if (document.getElementById('newTabNewsSection') && canvas.scrollTop > 80) fetchLiveNews(false);\n          var max = canvas.scrollHeight",1)
s=s.replace("              if (ns) ns.scrollIntoView({ behavior: 'smooth', block: 'start' });", "              if (ns) { ns.scrollIntoView({ behavior: 'smooth', block: 'start' }); fetchLiveNews(false); }",1)
s=s.replace("              newTabNewsCategory = btn.dataset.cat || 'All';", "              newTabNewsCategory = btn.dataset.cat || 'All';\n              fetchLiveNews(false);",1)
# Initial fetch on start page when already scrolled (e.g. after opening new tab) still is scroll-based.
p.write_text(s, encoding="utf-8")
print('Patched',p,'length',len(s),'live news:',s.count('fetchLiveNews'))
