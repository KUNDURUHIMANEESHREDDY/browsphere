(function () {
      'use strict';

      /* ========================================================================
         1. SAFE ESCAPING & PURE STRING-CONCATENATED PAGE TEMPLATES (ZERO NESTED BACKTICKS)
         ======================================================================== */
      function esc(str) {
        return String(str)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;');
      }

      var startPagePrefs = {
        showFavorites: true,
        showFrequentlyVisited: true,
        showPrivacyReport: true,
        showReadingList: false,
        surface: 'studio'
      };
      var startPageCustomizeOpen = false;

      var startPageFavorites = [
        {
          title: 'GitHub',
          url: 'github.com',
          svg: '<svg class="w-7 h-7" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.58 2 12.26c0 4.53 2.87 8.37 6.84 9.73.5.09.68-.22.68-.49 0-.24-.01-.89-.01-1.74-2.78.62-3.37-1.37-3.37-1.37-.45-1.18-1.11-1.49-1.11-1.49-.91-.64.07-.62.07-.62 1 .07 1.53 1.06 1.53 1.06.89 1.56 2.34 1.11 2.91.85.09-.66.35-1.11.63-1.37-2.22-.26-4.56-1.14-4.56-5.07 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.71 0 0 .84-.28 2.75 1.05A9.38 9.38 0 0112 6.84c.85 0 1.71.12 2.51.35 1.91-1.33 2.75-1.05 2.75-1.05.55 1.41.2 2.45.1 2.71.64.72 1.03 1.63 1.03 2.75 0 3.94-2.34 4.81-4.57 5.06.36.32.68.94.68 1.9 0 1.37-.01 2.48-.01 2.81 0 .27.18.59.69.49A10.03 10.03 0 0022 12.26C22 6.58 17.52 2 12 2z"/></svg>'
        },
        {
          title: 'Google',
          url: 'www.google.com',
          svg: '<span class="font-sans text-[22px] font-bold text-[#4285F4]">G</span>'
        },
        {
          title: 'YouTube',
          url: 'www.youtube.com',
          svg: '<svg class="w-7 h-7 text-[#FF0000]" viewBox="0 0 24 24" fill="currentColor"><path d="M21.58 7.19a2.5 2.5 0 00-1.76-1.77C18.25 5 12 5 12 5s-6.25 0-7.82.42a2.5 2.5 0 00-1.76 1.77C2 8.77 2 12 2 12s0 3.23.42 4.81a2.5 2.5 0 001.76 1.77C5.75 19 12 19 12 19s6.25 0 7.82-.42a2.5 2.5 0 001.76-1.77C22 15.23 22 12 22 12s0-3.23-.42-4.81zM10 15V9l5.2 3L10 15z"/></svg>'
        },
        {
          title: 'MDN Docs',
          url: 'developer.mozilla.org',
          svg: '<span class="font-mono text-[16px] font-bold tracking-tighter">MDN</span>'
        },
        {
          title: 'Wikipedia',
          url: 'en.wikipedia.org',
          svg: '<span class="font-serif text-[22px] font-semibold">W</span>'
        },
        {
          title: 'Hacker News',
          url: 'news.ycombinator.com',
          svg: '<span class="w-7 h-7 rounded-[6px] bg-[#FF6600] text-white font-sans text-[15px] font-bold flex items-center justify-center">Y</span>'
        }
      ];

      var readingList = [];

      var PRESET_PAGES = {
        'arxiv-review.org/quantum-2026': {
          url: 'arxiv-review.org/quantum-2026',
          title: 'Quantum Computing Innovations',
          group: 'Research & Papers',
          isArticle: true,
          readTime: '11 min read',
          summaryBullets: [
            '<strong>99.94% Two-Qubit Gate Fidelity:</strong> Achieved using flux-tunable coupler architectures across a 128-qubit superconducting lattice.',
            '<strong>4.2× Error Suppression:</strong> Scaling surface code distance from <em>d = 3</em> to <em>d = 7</em> suppresses logical error per cycle below threshold.',
            '<strong>412ns Neural Syndrome Decoder:</strong> On-chip FPGA + ML decoding prevents exponential classical syndrome backlog.'
          ],
          keyMetrics: '• Logical Error Rate: 1.8 × 10⁻⁶ per cycle at d = 7\n• Coherence Window (T₁): 410 μs median across lattice\n• Decoder Latency: 412 ns (vs 1,450 ns MWPM baseline)',
          critique: 'While distance-7 scaling is compelling on a 128-qubit array, scaling beyond 1,000 physical qubits will require modular cryogenic photonic interconnects to prevent readout thermal crosstalk.',
          renderHtml: function () {
            return (
              '<article class="max-w-[820px] mx-auto px-6 sm:px-12 py-8 sm:py-11">' +
                '<div class="mb-6 pb-5 border-b border-black/10 dark:border-white/10">' +
                  '<div class="flex flex-wrap items-center gap-2 text-xs font-medium text-[#007AFF] mb-2.5">' +
                    '<span class="px-2 py-0.5 rounded-md bg-[#007AFF]/10">arXiv:2604.09128 [quant-ph]</span>' +
                    '<span class="opacity-45">•</span>' +
                    '<span class="opacity-70">11 min read</span>' +
                    '<span class="opacity-45">•</span>' +
                    '<span class="text-emerald-500">● Active Page Context</span>' +
                  '</div>' +
                  '<h1 class="font-serif text-2xl sm:text-[32px] font-bold tracking-tight leading-[1.2] mb-3">' +
                    'Quantum Computing Innovations: Sub-Threshold Surface Code Scaling with Neural Syndrome Decoders' +
                  '</h1>' +
                  '<p class="text-xs opacity-65">Elena Rostova¹, Marcus Chen², Kenji Takahashi¹ — Center for Quantum Engineering</p>' +
                '</div>' +
                '<div class="grid grid-cols-1 md:grid-cols-3 gap-3.5 my-6 p-4 rounded-2xl bg-black/[0.03] dark:bg-white/[0.04] border border-black/10 dark:border-white/10">' +
                  '<div class="p-3 rounded-xl bg-white dark:bg-zinc-900 border border-black/5 dark:border-white/10">' +
                    '<div class="text-[11px] font-semibold mb-1">Logical Error Suppression</div>' +
                    '<svg viewBox="0 0 220 125" class="w-full h-auto">' +
                      '<line x1="28" y1="100" x2="205" y2="100" stroke="currentColor" stroke-opacity="0.25"/>' +
                      '<line x1="28" y1="15" x2="28" y2="100" stroke="currentColor" stroke-opacity="0.25"/>' +
                      '<path d="M34 25 Q 95 40, 196 90" fill="none" stroke="#007AFF" stroke-width="2.5"/>' +
                      '<path d="M34 40 Q 100 60, 196 95" fill="none" stroke="#AF52DE" stroke-width="2" stroke-dasharray="4 3"/>' +
                      '<circle cx="88" cy="48" r="3.5" fill="#007AFF"/>' +
                      '<circle cx="142" cy="69" r="3.5" fill="#007AFF"/>' +
                      '<circle cx="196" cy="90" r="3.5" fill="#007AFF"/>' +
                      '<text x="115" y="118" font-size="9" fill="currentColor" opacity="0.6" text-anchor="middle">Code Distance (d = 3 → 7)</text>' +
                    '</svg>' +
                  '</div>' +
                  '<div class="p-3 rounded-xl bg-white dark:bg-zinc-900 border border-black/5 dark:border-white/10">' +
                    '<div class="text-[11px] font-semibold mb-1">Decoder Latency (ns)</div>' +
                    '<svg viewBox="0 0 220 125" class="w-full h-auto">' +
                      '<line x1="28" y1="100" x2="205" y2="100" stroke="currentColor" stroke-opacity="0.25"/>' +
                      '<line x1="28" y1="42" x2="205" y2="42" stroke="#FF2D55" stroke-width="1" stroke-dasharray="3 3"/>' +
                      '<rect x="46" y="64" width="24" height="36" rx="3" fill="#007AFF"/>' +
                      '<rect x="90" y="54" width="24" height="46" rx="3" fill="#AF52DE"/>' +
                      '<rect x="134" y="47" width="24" height="53" rx="3" fill="#34C759"/>' +
                      '<rect x="174" y="20" width="20" height="80" rx="3" fill="#8E8E93"/>' +
                      '<text x="115" y="118" font-size="9" fill="currentColor" opacity="0.6" text-anchor="middle">Neural FPGA vs Classical MWPM</text>' +
                    '</svg>' +
                  '</div>' +
                  '<div class="p-3 rounded-xl bg-white dark:bg-zinc-900 border border-black/5 dark:border-white/10">' +
                    '<div class="text-[11px] font-semibold mb-1">Two-Qubit Gate Fidelity</div>' +
                    '<svg viewBox="0 0 220 125" class="w-full h-auto">' +
                      '<line x1="28" y1="100" x2="205" y2="100" stroke="currentColor" stroke-opacity="0.25"/>' +
                      '<path d="M34 98 C 72 98, 94 20, 124 20 C 154 20, 170 98, 200 98" fill="rgba(52,199,89,0.18)" stroke="#34C759" stroke-width="2.2"/>' +
                      '<text x="155" y="32" font-size="9" font-weight="bold" fill="#34C759">99.94%</text>' +
                      '<text x="115" y="118" font-size="9" fill="currentColor" opacity="0.6" text-anchor="middle">CZ Median Fidelity</text>' +
                    '</svg>' +
                  '</div>' +
                '</div>' +
                '<div class="reader-body font-serif text-[17px] leading-[1.78] space-y-5">' +
                  '<p><strong>Abstract —</strong> Fault-tolerant quantum computation requires physical error rates strictly below the surface code threshold alongside real-time classical decoding capable of matching syndrome extraction cycles. Here we demonstrate a 128-qubit superconducting processor utilizing flux-tunable couplers that suppresses logical error rates by <span class="smart-highlight">4.2× when scaling code distance from d = 3 to d = 7, maintaining a median two-qubit gate fidelity of 99.94%</span> across multi-round stabilizer measurements.</p>' +
                  '<h2 class="font-sans text-lg font-bold pt-2">1. Architecture &amp; Cryogenic Readout</h2>' +
                  '<p>Prior multi-qubit lattices suffered from frequency crowding and spectator phase errors during parallel stabilizer parity checks. By introducing asymmetric tunable transmon couplers with sub-nanosecond flux pulses, residual ZZ coupling is suppressed below 4 kHz during idle windows.</p>' +
                  '<p>Furthermore, our <span class="smart-highlight">hardware-accelerated neural syndrome decoder executes on a cryogenic-adjacent FPGA within 412 nanoseconds per cycle</span>, eliminating the exponential classical backlog that previously bottlenecked deep logical circuits.</p>' +
                '</div>' +
              '</article>'
            );
          }
        },

        'internal.acme.design/q3-memo': {
          url: 'internal.acme.design/q3-memo',
          title: 'Q3 Product Strategy Memo',
          group: 'Product & Strategy',
          isArticle: true,
          readTime: '6 min read',
          summaryBullets: [
            '<strong>Zero Right Sidebar Mandate:</strong> Eliminating permanent AI panels recovers +28% horizontal reading width and reduces cognitive fatigue.',
            '<strong>Single Top-Right Quick Chat:</strong> Anchored frosted popover keeps contextual intelligence one keystroke (⌘J) away without obstructing content.',
            '<strong>41% Faster Synthesis:</strong> Dogfooding across 140 engineers showed a 41% reduction in multi-tab research synthesis time.'
          ],
          keyMetrics: '• Horizontal Viewport Gain: +28% vs persistent split-pane\n• Synthesis Speedup: +41% across multi-tab workflows\n• On-Device Battery Impact: < 1.4% over 8-hour workday',
          critique: 'Discoverability of the top-right Lucia icon depends on subtle visual cues; pairing it with inline text-selection pills and ⌘J shortcut hints ensures immediate adoption.',
          renderHtml: function () {
            return (
              '<article class="max-w-[820px] mx-auto px-6 sm:px-12 py-8 sm:py-11">' +
                '<div class="mb-6 pb-5 border-b border-black/10 dark:border-white/10">' +
                  '<div class="flex items-center gap-2 text-xs font-medium text-amber-600 mb-2">' +
                    '<span class="px-2 py-0.5 rounded-md bg-amber-500/10">Executive Memo • Q3 Browser Architecture</span>' +
                    '<span class="opacity-45">•</span>' +
                    '<span class="opacity-65">6 min read</span>' +
                  '</div>' +
                  '<h1 class="font-serif text-2xl sm:text-[32px] font-bold tracking-tight leading-[1.2] mb-2">' +
                    'Q3 Product Strategy: Ambient Intelligence Without Permanent Sidebars' +
                  '</h1>' +
                  '<p class="text-xs opacity-60">Core Browser Experience Group • Cupertino Design Review</p>' +
                '</div>' +
                '<div class="reader-body font-serif text-[17px] leading-[1.78] space-y-5">' +
                  '<p><strong>Core Thesis —</strong> A desktop web browser is a window onto the open web, not a chat client with a tiny webpage squeezed beside it. <span class="smart-highlight">Locking a permanent AI sidebar onto the right edge steals 25% of horizontal viewport space</span> even when users are simply reading.</p>' +
                  '<div class="font-sans my-6 rounded-2xl border border-black/10 dark:border-white/10 overflow-hidden">' +
                    '<table class="w-full text-left text-xs">' +
                      '<thead class="bg-black/[0.04] dark:bg-white/[0.06] font-semibold">' +
                        '<tr>' +
                          '<th class="p-3">Experience Metric</th>' +
                          '<th class="p-3">Legacy Split-Sidebar</th>' +
                          '<th class="p-3 text-[#007AFF]">Anchored Top-Right Popover</th>' +
                        '</tr>' +
                      '</thead>' +
                      '<tbody class="divide-y divide-black/5 dark:divide-white/10">' +
                        '<tr><td class="p-3 font-medium">Usable Reading Canvas Width</td><td class="p-3 opacity-65">72%</td><td class="p-3 font-semibold text-emerald-600">100% (+28% gain)</td></tr>' +
                        '<tr><td class="p-3 font-medium">Time-to-First-Summary</td><td class="p-3 opacity-65">3.4 sec</td><td class="p-3 font-semibold text-emerald-600">0.4 sec (⌘J / Inline Pill)</td></tr>' +
                        '<tr><td class="p-3 font-medium">Deep-Reading Retention Score</td><td class="p-3 opacity-65">68 / 100</td><td class="p-3 font-semibold text-emerald-600">92 / 100</td></tr>' +
                      '</tbody>' +
                    '</table>' +
                  '</div>' +
                  '<p>By anchoring a single frosted-glass popover directly beneath the top-right toolbar trigger, <span class="smart-highlight">users retain full editorial focus while keeping page-aware AI synthesis one click away</span>.</p>' +
                '</div>' +
              '</article>'
            );
          }
        },

        'architectural-digest.jp/kyoto-timber': {
          url: 'architectural-digest.jp/kyoto-timber',
          title: 'Kyoto Architecture & Craft',
          group: 'Personal & Travel',
          isArticle: true,
          readTime: '8 min read',
          summaryBullets: [
            '<strong>Fastener-Free Kigumi Joinery:</strong> Precision-milled hinoki cypress joints deliver high seismic ductility without steel bolts.',
            '<strong>2.4m Cantilevered Engawa:</strong> Deep roof eaves shade summer zenith sun while inviting cool cross-breezes from moss gardens.',
            '<strong>62% Lower Embodied Carbon:</strong> Locally sourced Kitayama cedar outperforms reinforced concrete in lifecycle sustainability.'
          ],
          keyMetrics: '• Embodied Carbon Reduction: -62% vs concrete\n• Cantilever Eave Depth: 2.4 meters passive cooling\n• Structural Lifespan: 150+ years modular disassembly',
          critique: 'Traditional cypress joinery achieves remarkable longevity and seismic performance, though scaling to mid-rise urban housing depends on hybrid CNC prefabrication.',
          renderHtml: function () {
            return (
              '<article class="max-w-[820px] mx-auto px-6 sm:px-12 py-8 sm:py-11">' +
                '<div class="mb-6 pb-5 border-b border-black/10 dark:border-white/10">' +
                  '<div class="flex items-center gap-2 text-xs font-medium text-emerald-600 mb-2">' +
                    '<span class="px-2 py-0.5 rounded-md bg-emerald-500/10">Spatial Design • Northern Kyoto</span>' +
                    '<span class="opacity-45">•</span>' +
                    '<span class="opacity-65">8 min read</span>' +
                  '</div>' +
                  '<h1 class="font-serif text-2xl sm:text-[32px] font-bold tracking-tight leading-[1.2] mb-2">' +
                    'Quiet Precision: Contemporary Timber Pavilions in Higashiyama' +
                  '</h1>' +
                  '<p class="text-xs opacity-60">By Haruki Sato • Architectural Field Notes</p>' +
                '</div>' +
                '<div class="reader-body font-serif text-[17px] leading-[1.78] space-y-5">' +
                  '<p>Along the cedar-forested slopes of Kitayama, contemporary Kyoto architects are reviving centuries-old joinery traditions for modern residential pavilions. Rather than concealing structural framing, <span class="smart-highlight">primary timber columns interlock via precision-milled Kigumi mortise-and-tenon joints without a single steel fastener</span>.</p>' +
                  '<p>The spatial heart of each pavilion is the <em>engawa</em>—a sheltered perimeter veranda beneath a 2.4-meter cantilevered cypress eave that mediates between quiet tatami rooms and rain-washed moss gardens.</p>' +
                '</div>' +
              '</article>'
            );
          }
        },

        'ml-explore.github.io/mlx': {
          url: 'ml-explore.github.io/mlx',
          title: 'Apple MLX Local Inference',
          group: 'Research & Papers',
          isArticle: true,
          readTime: '5 min read',
          summaryBullets: [
            '<strong>Zero-Copy Unified Memory:</strong> CPU and GPU kernels operate directly on shared Apple Silicon DRAM with zero host-to-device transfers.',
            '<strong>118.4 Tokens/Sec Throughput:</strong> 4-bit affine quantization (group size 64) delivers sub-35ms time-to-first-token on M4 Max.',
            '<strong>Lazy Computation Graphs:</strong> Kernel fusion happens automatically upon <code>mx.eval()</code> invocation.'
          ],
          keyMetrics: '• Token Generation Speed: 118.4 tok/s (8B 4-bit)\n• Time-to-First-Token: 34 ms (512-token context)\n• Peak Unified RAM: 4.85 GB',
          critique: 'Unified memory eliminates PCIe transfer bottlenecks completely for single-user desktop inference, though high-concurrency server batching remains bandwidth-bound.',
          renderHtml: function () {
            return (
              '<article class="max-w-[820px] mx-auto px-6 sm:px-12 py-8 sm:py-11">' +
                '<div class="mb-6 pb-5 border-b border-black/10 dark:border-white/10">' +
                  '<div class="flex items-center gap-2 text-xs font-medium text-purple-600 mb-2">' +
                    '<span class="px-2 py-0.5 rounded-md bg-purple-500/10">Apple Silicon ML • v0.24 Reference</span>' +
                    '<span class="opacity-45">•</span>' +
                    '<span class="opacity-65">5 min read</span>' +
                  '</div>' +
                  '<h1 class="font-serif text-2xl sm:text-[32px] font-bold tracking-tight leading-[1.2] mb-2">' +
                    'Zero-Copy On-Device LLM Inference with MLX Unified Memory' +
                  '</h1>' +
                  '<p class="text-xs opacity-60">MLX Core Documentation • Local Browser Runtime</p>' +
                '</div>' +
                '<div class="reader-body font-serif text-[17px] leading-[1.78] space-y-5">' +
                  '<p>MLX is an array framework engineered for Apple silicon. Unlike discrete GPU architectures, <span class="smart-highlight">arrays in MLX live in unified memory and can be operated on by CPU and GPU kernels with zero data movement</span>.</p>' +
                  '<div class="relative font-mono text-xs bg-zinc-900 text-zinc-100 p-4 rounded-xl overflow-x-auto">' +
                    '<button type="button" data-action="copy-code" class="absolute top-2.5 right-2.5 px-2 py-1 rounded bg-white/10 hover:bg-white/20 text-[10px] font-sans">Copy</button>' +
                    '<pre><code>import mlx.core as mx\nfrom mlx_lm import load, stream_generate\n\nmodel, tokenizer = load("mlx-community/Llama-3.2-8B-Instruct-4bit")\nfor token in stream_generate(model, tokenizer, prompt="Summarize page:", max_tokens=256):\n    print(token.text, end="", flush=True)</code></pre>' +
                  '</div>' +
                  '<p>With 4-bit affine quantization and KV-cache rotation, <span class="smart-highlight">local models achieve 118.4 tokens/sec directly inside Browsphere’s sandboxed Neural Engine runtime</span>.</p>' +
                '</div>' +
              '</article>'
            );
          }
        },

        'browsphere://start': {
          url: 'browsphere://start',
          title: 'New Tab',
          group: 'Open Tabs',
          isArticle: false,
          readTime: 'New Tab',
          summaryBullets: [
            '<strong>Oversized Greeting &amp; Bottom-Middle Chat Box:</strong> Focused light-first workspace with Ask and Browse modes.',
            '<strong>Quick Preview Thumbnails:</strong> Hover over source verification pills or any News story to pop out a mini-screen preview without opening ten tabs.',
            '<strong>Top-Right Chat History Tab:</strong> Slide-over right drawer for revisiting, resuming, or clearing past chats.'
          ],
          keyMetrics: '• Layout: Editorial New Tab + Hover Mini-Screen Previews + Top-Right Chat History\n• Active Stories: 6\n• Theme: Clean Light / Dark Sync',
          critique: 'Minimalist new tab surface with a single focal greeting, bottom-middle chat box, hover-to-preview source thumbnails and news mini-screens, and top-right chat history.',
          renderHtml: function () {
            if (getActiveTab()?.viewingArchivedChat && getActiveTab()?.chatThreadId) return renderNewTabConversation();
            var hr = new Date().getHours();
            var greetingText = hr < 12 ? 'Good morning.' : (hr < 18 ? 'Good afternoon.' : 'Good evening.');

            var categories = ['All', 'AI & Systems', 'Engineering', 'Design', 'Science'];
            var catTabsHtml = categories.map(function (cat) {
              var active = newTabNewsCategory === cat;
              var cls = active
                ? 'text-[#141416] dark:text-[#F5F5F7] font-semibold border-b-2 border-[#007AFF] pb-2'
                : 'text-[#6E6E73] dark:text-[#98989D] hover:text-[#141416] dark:hover:text-[#F5F5F7] pb-2 border-b-2 border-transparent';
              return '<button type="button" data-action="nt-news-cat" data-cat="' + esc(cat) + '" class="text-[13px] transition ' + cls + '">' + esc(cat) + '</button>';
            }).join('');

            var filteredNews = newTabNewsItems.filter(function (item) {
              return newTabNewsCategory === 'All' || item.category === newTabNewsCategory;
            });

            var leadStory = filteredNews[0] || null;
            var restStories = filteredNews.slice(1);

            var leadHtml = leadStory ? (
              '<button type="button" data-action="navigate" data-url="' + esc(leadStory.url) + '" data-preview-url="' + esc(leadStory.url) + '" class="text-left group block w-full py-2 focus:outline-none">' +
                '<div class="rounded-[12px] overflow-hidden border border-black/[0.08] dark:border-white/[0.1] bg-[#FAFAFC] dark:bg-[#1B1B1F] h-[140px] mb-4 p-3 flex items-center justify-center">' +
                  leadStory.thumbSvg +
                '</div>' +
                '<div class="text-[12px] font-medium text-[#007AFF] dark:text-[#0A84FF] mb-1.5">' + esc(leadStory.category) + ' &mdash; ' + esc(leadStory.source) + '</div>' +
                '<h3 class="font-serif text-[27px] sm:text-[32px] leading-[1.16] tracking-[-0.015em] font-normal group-hover:text-[#007AFF] dark:group-hover:text-[#0A84FF] transition">' + esc(leadStory.title) + '</h3>' +
                '<p class="text-[13.5px] leading-[1.62] text-[#55555A] dark:text-[#A1A1A6] mt-2.5 max-w-[54ch]">' + esc(leadStory.dek) + '</p>' +
                '<div class="mt-3.5 text-[12px] text-[#6E6E73] dark:text-[#98989D] flex items-center justify-between">' +
                  '<span>' + esc(leadStory.readTime) + ' &nbsp;&middot;&nbsp; ' + esc(leadStory.timeAgo) + '</span>' +
                  '<span class="text-[11px] opacity-70">Hover to preview</span>' +
                '</div>' +
              '</button>'
            ) : '';

            var listHtml = restStories.map(function (st) {
              return (
                '<button type="button" data-action="navigate" data-url="' + esc(st.url) + '" data-preview-url="' + esc(st.url) + '" class="nt-news-row w-full text-left py-3.5 px-2 -mx-2 rounded-lg flex items-start gap-3.5 focus:outline-none group">' +
                  '<div class="qp-news-thumb mt-0.5">' + st.thumbSvg + '</div>' +
                  '<div class="min-w-0 flex-1">' +
                    '<div class="flex items-baseline justify-between gap-3">' +
                      '<span class="text-[11.5px] font-medium text-[#6E6E73] dark:text-[#98989D] truncate">' + esc(st.category) + ' &mdash; ' + esc(st.source) + '</span>' +
                      '<span class="text-[11px] text-[#8E8E93] shrink-0">' + esc(st.timeAgo) + '</span>' +
                    '</div>' +
                    '<div class="text-[14.5px] font-medium leading-snug mt-0.5 group-hover:text-[#007AFF] dark:group-hover:text-[#0A84FF] transition">' + esc(st.title) + '</div>' +
                    '<p class="text-[12px] text-[#6E6E73] dark:text-[#98989D] mt-1 line-clamp-2 leading-relaxed">' + esc(st.dek) + '</p>' +
                  '</div>' +
                '</button>'
              );
            }).join('');

            // Chat replies appear in the main-tab conversation view, never in a fake source banner.
            var replyBannerHtml = '';

            var modeAskCls = newTabChatMode === 'ask'
              ? 'bg-black/[0.07] dark:bg-white/[0.12] text-[#141416] dark:text-white font-semibold'
              : 'text-[#6E6E73] dark:text-[#98989D] hover:text-[#141416] dark:hover:text-white';
            var modeBrowseCls = newTabChatMode === 'browse'
              ? 'bg-black/[0.07] dark:bg-white/[0.12] text-[#141416] dark:text-white font-semibold'
              : 'text-[#6E6E73] dark:text-[#98989D] hover:text-[#141416] dark:hover:text-white';

            return (
              '<div class="nt-page w-full">' +
                '<section class="nt-hero" aria-label="New Tab Home">' +
                  '<!-- Top-Right Chat History Tab -->' +
                  '<div class="flex items-center justify-end">' +
                    '<button type="button" data-action="toggle-nt-history" class="nt-history-tab-btn px-3.5 py-1.5 rounded-[10px] flex items-center gap-2 text-[12.5px] font-medium">' +
                      '<svg class="w-4 h-4 opacity-75" fill="none" stroke="currentColor" stroke-width="1.75" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 2m6-2a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>' +
                      '<span>Chat History</span>' +
                      '<span class="text-[11px] px-1.5 py-0.5 rounded-md bg-black/[0.06] dark:bg-white/[0.1] opacity-75">' + newTabChatHistory.length + '</span>' +
                    '</button>' +
                  '</div>' +

                  '<!-- Centered Oversized Greeting -->' +
                  '<div class="flex-1 flex flex-col items-center justify-center text-center px-4 -mt-6">' +
                    '<h1 class="nt-greeting">' + esc(greetingText) + '</h1>' +
                    '<p class="text-[14px] text-[#6E6E73] dark:text-[#98989D] mt-3 max-w-[46ch]">Ask Lucia a question, jump to any address, or scroll down to explore the latest stories.</p>' +
                  '</div>' +

                  '<!-- Bottom-Middle Chat Box + Scroll Cue -->' +
                  '<div class="w-full max-w-[660px] mx-auto pb-1">' +
                    replyBannerHtml +
                    '<form id="newTabChatForm" class="nt-chatbox rounded-[16px] p-2.5">' +
                      '<div class="flex items-end gap-2">' +
                        '<textarea id="newTabChatInput" rows="1" placeholder="' + (newTabChatMode === 'ask' ? 'Ask Lucia anything or start a task...' : 'Enter a website or search query...') + '" class="flex-1 resize-none bg-transparent px-2.5 py-1.5 text-[14px] leading-relaxed focus:outline-none max-h-[140px]"></textarea>' +
                        '<button type="button" data-action="nt-run-agent" class="main-chat-task-btn" title="Run a policy-gated multi-step browser task">Run task</button>' +
                        '<button type="submit" class="px-4 py-2 rounded-[10px] bg-[#007AFF] text-white text-[13px] font-semibold hover:brightness-110 transition shrink-0">' + (newTabChatMode === 'ask' ? 'Send' : 'Go') + '</button>' +
                      '</div>' +
                      '<div class="flex items-center justify-between pt-2 mt-1.5 border-t border-black/[0.06] dark:border-white/[0.08] px-1">' +
                        '<div class="flex items-center gap-1">' +
                          '<button type="button" data-action="nt-set-mode" data-mode="ask" class="px-2.5 py-1 rounded-[7px] text-[12px] transition ' + modeAskCls + '">Ask</button>' +
                          '<button type="button" data-action="nt-set-mode" data-mode="browse" class="px-2.5 py-1 rounded-[7px] text-[12px] transition ' + modeBrowseCls + '">Browse</button>' +
                        '</div>' +
                        '<span class="text-[11.5px] text-[#8E8E93]">Enter to send &middot; Shift+Enter for new line</span>' +
                      '</div>' +
                    '</form>' +

                    '<div class="mt-5 flex justify-center">' +
                      '<button type="button" data-action="nt-scroll-news" class="text-[12px] text-[#6E6E73] dark:text-[#98989D] hover:text-[#141416] dark:hover:text-white flex items-center gap-1.5 transition">' +
                        '<span>Scroll for news</span>' +
                        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="1.8" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>' +
                      '</button>' +
                    '</div>' +
                  '</div>' +
                '</section>' +

                '<!-- Scrollable News Page Below the Fold -->' +
                '<section id="newTabNewsSection" aria-label="News" class="nt-news-section max-w-[1040px] mx-auto px-6 sm:px-10 py-14">' +
                  '<div class="flex flex-col sm:flex-row sm:items-end justify-between gap-4 border-b border-black/10 dark:border-white/10 pb-1 mb-9">' +
                    '<div>' +
                      '<div class="flex items-center gap-3"><h2 class="font-serif text-[32px] leading-tight tracking-[-0.015em] font-normal">News</h2><span class="inline-flex items-center gap-1.5 text-[10px] font-semibold tracking-wide text-emerald-600 dark:text-emerald-400"><span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>' + (newTabNewsLoading ? 'UPDATING' : (newTabNewsError ? 'OFFLINE' : (newTabNewsLastFetch ? 'LIVE' : 'READY'))) + '</span></div>' +
                    '</div>' +
                    '<div class="flex items-center gap-5 overflow-x-auto">' + catTabsHtml + '</div>' +
                  '</div>' +

                  '<div class="grid grid-cols-1 lg:grid-cols-12 gap-10 items-start">' +
                    '<div class="lg:col-span-6 lg:pr-6 lg:border-r border-black/[0.08] dark:border-white/[0.09]">' +
                      (leadHtml || '<p class="text-[13px] opacity-65 py-6">' + (newTabNewsLoading ? 'Loading current headlines…' : (newTabNewsError || (newTabNewsItems.length ? 'No stories in this category yet.' : 'Scroll here to load current headlines.'))) + '</p>') +
                    '</div>' +
                    '<div class="lg:col-span-6 divide-y-0">' +
                      (listHtml || (newTabNewsError ? '<p class="text-[13px] opacity-65 py-6">' + esc(newTabNewsError) + '</p>' : '')) +
                    '</div>' +
                  '</div>' +
                '</section>' +
              '</div>'
            );
          }
        }      };

      /* ========================================================================
         2. BROWSER STATE
         ======================================================================== */
      var tabs = [
        { id: 'tab-1', group: 'Open Tabs', history: ['browsphere://start'], historyIndex: 0 }
      ];
      var activeTabId = 'tab-1';
      var sidebarSection = 'tabs';
      var isPrivateMode = false;
      var normalTabsBeforePrivate = null;
      var isReaderMode = false;
      var readerFontSize = 17;
      var readerTheme = 'white';
      var isTtsPlaying = false;
      var isVoiceDictating = false;
      var newTabChatMode = 'ask';
      var newTabNewsCategory = 'All';
      var newTabNewsLoading = false;
      var newTabNewsError = '';
      var newTabNewsLastFetch = 0;
      var newTabNewsStream = null;
      var newTabNewsStreamReady = false;
      var newTabNewsTimer = null;
      var newTabNewsDebounce = null;
      var newTabNewsCache = new Map();
      var newTabNewsIds = '';
      var newTabLastReply = null;
      var newTabHistoryOpen = false;
      var qpHoverTimer = null;
      var qpHideTimer = null;
      var qpActiveUrl = '';

      // Real user threads only. No fabricated pre-filled conversations or sources.
      var newTabChatHistory = [];
      // Agent run events can arrive before Tauri resolves start_agent_run.
      var pendingLuciaEvents = [];
      var luciaIntentPending = false;

      var newTabNewsItems = [];
      var selectedTextCache = '';

      var bookmarks = [];

      var browsingHistory = [];

      var downloads = [];

      // Persist only ordinary tab URLs; private navigation never enters the saved session.
      function saveBrowserSession() {
        if (window.BrowsphereState) {
          window.BrowsphereState.saveSession(localStorage, tabs, activeTabId, isPrivateMode);
        }
      }
      function saveBrowserLibrary() {
        if (window.BrowsphereState) {
          window.BrowsphereState.saveLibrary(localStorage, bookmarks, readingList);
        }
      }
      function restoreBrowserSession() {
        if (!window.BrowsphereState) return;
        var restored = window.BrowsphereState.load(localStorage);
        tabs = restored.tabs;
        activeTabId = restored.activeTabId;
        bookmarks = restored.bookmarks;
        readingList = restored.readingList;
      }

      var findMatches = [];
      var activeFindIndex = -1;

      var browserSettings = {
        searchEngine: 'Google',
        homepageUrl: 'browsphere://start',
        newTabsOpenWith: 'Start Page',
        defaultZoom: 100,
        tabLayout: 'Compact',
        autoCloseTabs: 'Manually',
        smartAiGrouping: true,
        preventCrossSiteTracking: true,
        hideIpFromTrackers: true,
        blockAllCookies: false,
        fraudulentWarning: true,
        aiEngine: 'Apple Intelligence Local 3B',
        aiTone: 'Concise',
        inlineSelectionPopover: true,
        continuousContextMemory: true,
        autoMemoryEnabled: (function () {
          try { return localStorage.getItem('browsphere-auto-memory') !== 'off'; }
          catch (_) { return true; }
        })(),
        // No browser extensions are installed; do not claim third-party extensions work.
        extensions: [],
        autofillPasswords: true,
        // Native builds load actual saved credentials from Windows Credential Manager.
        savedLogins: [],
        showFullWebsiteAddress: false,
        showDeveloperMenu: false,
        reducedMotion: false
      };

      var activeSettingsPane = 'general';
      var selectedExtensionIdx = 0;
      var revealedPasswords = {};
      var passwordSearchQuery = '';
      var activeInspectorTab = 'elements';
      var nativeRouterConfig = {
        ollama_url: 'http://127.0.0.1:11434',
        ollama_model: 'qwen3.5:4b',
        litellm_url: 'http://127.0.0.1:4000/v1/chat/completions',
        litellm_model: 'gpt-4o-mini',
        litellm_api_key: '',
        domain_overrides: {},
        prefer_local_for_simple: true,
        lucia_cloud_only: false,
        cloud_model_catalogs: {}
      };
      var nativeAwsConfig = {
        profile: 'default', region: 'us-east-1', bedrock_model: 'amazon.nova-lite-v1:0',
        bedrock_enabled: false, s3_bucket: '', s3_key: 'browsphere/settings-backup.json',
        dynamodb_table: '', sync_namespace: 'default', cognito_client_id: '', lambda_function: ''
      };
      var nativeAwsStatus = '';
      var nativeAwsSession = null;
      var nativeAwsServices = [];
      var nativeAwsAccountHint = '';
      var nativeAwsConnecting = false;
      var nativeHasLiteLlmKey = false;
      var nativeOllamaModels = [];
      var nativeOllamaModelsError = 'Checking Ollama models...';
      var nativeModelTestStatus = '';
      var nativeCloudFetchStatus = '';
      var nativeCredentialsMeta = [];
      var nativeRevealedCache = {};
      var nativeRuns = [];
      var nativeTraces = [];
      var nativeMacros = [];
      var nativeAutomations = [];
      var draftAutomationSteps = [];
      var editingAutomationId = null;
      var traceFilterRunId = 'all';
      var traceFilterRisk = 'all';
      var nativeItpTotal = 0;
      var nativeItpBreakdown = [];
      var inspectorConsoleLogs = [
        { level: 'info', text: '[Apple Intelligence] Local 3B MLX runtime initialized in 38ms.' },
        { level: 'log', text: '[WebKit ITP] Blocked 14 cross-site tracking requests on active document.' }
      ];

      /* ========================================================================
         3. CENTRAL DELEGATED EVENT LISTENERS
         ======================================================================== */
      function bindAllEvents() {
        document.addEventListener('change', function (event) {
          if (event.target && event.target.id === 'cfgPreferLocalToggle') {
            changeNativeAiPreference(event.target.checked ? 'ollama' : 'litellm');
          }
          if (event.target && event.target.id === 'cfgFetchedCloudModels' && event.target.value) {
            var selectedModel = event.target.value;
            var modelInput = document.getElementById('cfgLiteLlmModel');
            if (modelInput) modelInput.value = selectedModel;
            saveNativeRouterSettings(true).then(function (saved) {
              if (saved) {
                showModelTestStatus('Selected cloud model: ' + selectedModel);
                showHudToast('Cloud model set to ' + selectedModel);
              }
            });
          }
          if (event.target && event.target.id === 'cfgLiteLlmUrl') {
            // A changed endpoint must never display the previous provider's cached models.
            showCloudFetchStatus('Click Fetch Models to discover models for this endpoint.');
            var cachedSelect = document.getElementById('cfgFetchedCloudModels');
            if (cachedSelect) {
              cachedSelect.innerHTML = '<option value="">Fetch models for this endpoint</option>';
              cachedSelect.disabled = true;
            }
          }
          if (event.target && event.target.id === 'cfgProviderPreset') {
            var preset = (window.browsphereCloudPresets || {})[event.target.value];
            if (preset) {
              document.getElementById('cfgLiteLlmUrl').value = preset[0];
              document.getElementById('cfgLiteLlmModel').value = preset[1];
              nativeCloudFetchStatus = '';
              changeNativeAiPreference('litellm').then(function () {
                if (activeSettingsPane === 'ai') rerenderAiPreservingDraft();
              });
            }
          }
        });
        document.addEventListener('click', function (e) {
          var highlightEl = e.target.closest('.smart-highlight');
          if (highlightEl) {
            clickSmartHighlight(highlightEl);
            return;
          }

          var btn = e.target.closest('[data-action]');
          if (!btn) return;

          var action = btn.dataset.action;
          switch (action) {
            case 'toast':
              showHudToast(btn.dataset.msg || 'Done');
              break;
            case 'toggle-fullscreen':
              toggleFullscreenWindow();
              break;
            case 'toggle-sidebar':
              toggleSidebar();
              break;
            case 'history-back':
              goHistory(-1);
              break;
            case 'history-forward':
              goHistory(1);
              break;
            case 'go-home':
              navigateToUrl(browserSettings.homepageUrl || 'browsphere://start');
              break;
            case 'toggle-private':
              togglePrivateBrowsing();
              break;
            case 'toggle-theme':
              toggleThemeMode();
              break;
            case 'toggle-popover':
              togglePopover(btn.dataset.target);
              break;
            case 'close-popovers':
              closeAllPopovers();
              break;
            case 'toggle-reader':
              toggleReaderMode();
              break;
            case 'exit-reader':
              toggleReaderMode(false);
              break;
            case 'reader-font':
              adjustReaderFont(Number(btn.dataset.delta || 0));
              break;
            case 'reader-theme':
              setReaderTheme(btn.dataset.rtheme || 'white');
              break;
            case 'toggle-tts':
              togglePageTts();
              break;
            case 'reload-page':
              reloadActivePage();
              break;
            case 'new-tab':
              createNewTab();
              break;
            case 'switch-tab':
              switchToTab(btn.dataset.tabId);
              break;
            case 'close-tab':
              e.stopPropagation();
              closeTab(btn.dataset.tabId);
              break;
            case 'overview-switch-tab':
              switchToTab(btn.dataset.tabId);
              toggleTabOverview(false);
              break;
            case 'overview-new-tab':
              createNewTab();
              toggleTabOverview(false);
              break;
            case 'toggle-tab-overview':
              toggleTabOverview();
              break;
            case 'close-tab-overview':
              toggleTabOverview(false);
              break;
            case 'sidebar-section':
              switchSidebarSection(btn.dataset.section);
              break;
            case 'navigate':
              navigateToUrl(btn.dataset.url);
              break;
            case 'bookmark-current':
              bookmarkCurrentPage();
              break;
            case 'reading-add':
              addToReadingList();
              break;
            case 'open-reading':
              openReadingItem(Number(btn.dataset.index));
              break;
            case 'clear-history':
              browsingHistory = [];
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.clearHistory().catch(function () {});
              }
              renderSidebarSection();
              showHudToast('History cleared');
              break;
            case 'agent-pause':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.togglePause().then(function (isPaused) {
                  var b = document.getElementById('chatPauseAgentBtn');
                  if (b) b.textContent = isPaused ? 'Resume' : 'Pause';
                  showHudToast(isPaused ? 'Agent paused' : 'Agent resumed');
                }).catch(function () {});
              }
              break;
            case 'agent-stop':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.cancelRun().then(function () {
                  var b = document.getElementById('chatPauseAgentBtn');
                  if (b) b.textContent = 'Pause';
                  showHudToast('Agent run cancelled');
                }).catch(function () {});
              }
              break;
            case 'find-step':
              stepFindMatch(Number(btn.dataset.delta || 1));
              break;
            case 'close-find':
              toggleFindBar(false);
              break;
            case 'inline-ai':
              handleInlineAiAction(btn.dataset.itype);
              break;
            case 'chat-chip':
              submitChatPrompt(btn.dataset.prompt);
              break;
            case 'start-ai-prompt':
              if (getActiveTab().history[getActiveTab().historyIndex] === 'browsphere://start') {
                submitNewTabChat(btn.dataset.prompt);
              } else {
                togglePopover('aiChatPopover', true);
                submitChatPrompt(btn.dataset.prompt);
              }
              break;
            case 'toggle-nt-history':
              toggleNewTabHistoryDrawer();
              break;
            case 'nt-new-chat':
              getActiveTab().chatThreadId = null;
              getActiveTab().viewingArchivedChat = false;
              toggleNewTabHistoryDrawer(false);
              renderActiveTab();
              var nci = document.getElementById('newTabChatInput');
              if (nci) nci.focus();
              break;
            case 'nt-clear-history':
              newTabChatHistory = [];
              getActiveTab().chatThreadId = null;
              getActiveTab().viewingArchivedChat = false;
              renderNewTabHistoryDrawer();
              renderActiveTab();
              showHudToast('Chat history cleared');
              break;
            case 'nt-load-history':
              loadNewTabHistoryItem(btn.dataset.hid);
              break;
            case 'nt-run-agent':
              startNewTabAgentTask();
              break;
            case 'nt-view-task-tab':
              if (tabs.some(function (tab) { return tab.id === btn.dataset.tabId; })) {
                switchToTab(btn.dataset.tabId);
              } else {
                showHudToast('The Lucia task tab has been closed');
              }
              break;
            case 'nt-task-stop':
              if (window.aetherBridge?.isNative) window.aetherBridge.cancelRun();
              break;
            case 'nt-task-approve':
            case 'nt-task-reject':
              answerLuciaTaskApproval(btn.dataset.approvalId, btn.dataset.action === 'nt-task-approve');
              break;
            case 'nt-set-mode':
              newTabChatMode = btn.dataset.mode === 'browse' ? 'browse' : 'ask';
              renderActiveTab();
              var mi = document.getElementById('newTabChatInput');
              if (mi) mi.focus();
              break;
            case 'nt-dismiss-reply':
              newTabLastReply = null;
              renderActiveTab();
              break;
            case 'nt-scroll-news':
              var ns = document.getElementById('newTabNewsSection');
              if (ns) { ns.scrollIntoView({ behavior: 'smooth', block: 'start' }); startLiveNewsStream(); }
              break;
            case 'nt-news-cat':
              newTabNewsCategory = btn.dataset.cat || 'All';
              fetchLiveNews(false);
              renderActiveTab();
              var ns2 = document.getElementById('newTabNewsSection');
              if (ns2) ns2.scrollIntoView({ behavior: 'auto', block: 'start' });
              break;
            case 'qp-open-current':
              hideQuickPreview(true);
              if (btn.dataset.url) navigateToUrl(btn.dataset.url);
              break;
            case 'qp-open-new-tab':
              hideQuickPreview(true);
              if (btn.dataset.url) createNewTab(btn.dataset.url);
              break;
            case 'qp-ask-ai':
              hideQuickPreview(true);
              if (btn.dataset.url) {
                var pMeta = getPreviewMeta(btn.dataset.url);
                if (getActiveTab().history[getActiveTab().historyIndex] === 'browsphere://start') {
                  submitNewTabChat('Explain the key claims from ' + pMeta.title + ' (' + pMeta.url +
                    '). If you cannot open the source, say so.');
                } else {
                  togglePopover('aiChatPopover', true);
                  submitChatPrompt('Explain the key claims from ' + pMeta.title + ' (' + pMeta.url + ')');
                }
              }
              break;
            case 'clear-chat':
              clearChatMessages();
              break;
            case 'reset-chat-window':
              resetChatWindowLayout();
              break;
            case 'toggle-voice':
              toggleVoiceDictation();
              break;
            case 'sim-download':
              triggerSimulatedDownload();
              break;
            case 'clear-downloads':
              downloads = [];
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.clearDownloads().catch(function () {});
              }
              renderDownloadsList();
              break;
            case 'cancel-download':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.dlId) {
                window.aetherBridge.cancelDownload(btn.dataset.dlId).catch(function () {});
                showHudToast('Download cancelled');
              }
              break;
            case 'share-service':
              closeAllPopovers();
              showHudToast('Prepared for ' + (btn.dataset.service || 'Share'));
              break;
            case 'copy-link':
              copyActivePageLink();
              break;
            case 'copy-code':
              copySnippet(btn);
              break;
            case 'toggle-startpage-customize':
              startPageCustomizeOpen = !startPageCustomizeOpen;
              renderActiveTab();
              break;
            case 'set-startpage-surface':
              startPagePrefs.surface = btn.dataset.surface || 'studio';
              renderActiveTab();
              break;
            case 'add-startpage-favorite':
              addStartPageFavorite();
              break;
            case 'open-settings':
              openSettingsModal(btn.dataset.pane || 'general');
              break;
            case 'close-settings':
              closeSettingsModal();
              break;
            case 'settings-pane':
              switchSettingsPane(btn.dataset.pane);
              break;
            case 'set-tab-layout':
              updateSetting('tabLayout', btn.dataset.layout);
              break;
            case 'select-extension':
              selectedExtensionIdx = Number(btn.dataset.extSelect || 0);
              renderSettingsPane();
              break;
            case 'set-current-home':
              var cur = getActiveTab().history[getActiveTab().historyIndex];
              updateSetting('homepageUrl', cur);
              showHudToast('Homepage set to ' + cur);
              break;
            case 'clear-site-data':
              browsingHistory = [];
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.clearHistory().catch(function () {});
              }
              renderSidebarSection();
              showHudToast('History & website data cleared');
              break;
            case 'save-router-settings':
              saveNativeRouterSettings();
              break;
            case 'save-aws-settings':
              saveNativeAwsSettings();
              break;
            case 'aws-connect':
              connectNativeAwsAccount();
              break;
            case 'aws-test':
            case 'aws-s3-backup':
            case 'aws-s3-restore':
            case 'aws-dynamo-push':
            case 'aws-dynamo-pull':
            case 'aws-cognito-login':
            case 'aws-cognito-logout':
            case 'aws-lambda-invoke':
              runNativeAwsAction(btn.dataset.action);
              break;
            case 'open-diagnostics':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.openDiagnosticsFolder().then(function () {
                  showHudToast('Diagnostics folder opened');
                }).catch(function () { showHudToast('Could not open diagnostics folder'); });
              } else showHudToast('Open Logs requires Browsphere desktop');
              break;
            case 'export-diagnostics':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.exportDiagnostics().then(function (path) {
                  showHudToast('Diagnostics exported: ' + path);
                }).catch(function () { showHudToast('Could not export diagnostics'); });
              } else showHudToast('Export requires Browsphere desktop');
              break;
            case 'lucia-diag-refresh':
              if (window.LuciaDiagnosticsDashboard) window.LuciaDiagnosticsDashboard.refresh();
              break;
            case 'lucia-diag-select':
              if (window.LuciaDiagnosticsDashboard) window.LuciaDiagnosticsDashboard.select(btn.dataset.runId);
              break;
            case 'lucia-diag-filter':
              if (window.LuciaDiagnosticsDashboard) window.LuciaDiagnosticsDashboard.filter(btn.dataset.filter);
              break;
            case 'mcp-lucia-enable':
            case 'mcp-lucia-revoke':
            case 'mcp-oauth-begin':
            case 'mcp-oauth-copy':
            case 'mcp-oauth-finish':
            case 'mcp-oauth-forget':
            case 'mcp-marketplace':
            case 'mcp-connect':
            case 'mcp-disconnect':
            case 'mcp-call':
              handleMcpSettingsAction(btn.dataset.action);
              break;
            case 'test-model-connection':
              testNativeModelConnection(null);
              break;
            case 'test-local-model':
              testNativeModelConnection('local');
              break;
            case 'use-local-lucia-fallback':
              useInstalledLocalModelForLucia();
              break;
            case 'use-local-lucia-profile':
              useInstalledLocalModelForLucia(btn.dataset.model);
              break;
            case 'test-cloud-model':
              testNativeModelConnection('cloud');
              break;
            case 'test-cloud-agent':
              testNativeModelConnection('cloud-agent');
              break;
            case 'fetch-cloud-models':
              fetchNativeCloudModels();
              break;
            case 'set-ai-preferred':
              changeNativeAiPreference(btn.dataset.engine);
              break;
            case 'refresh-ai-models':
              refreshNativeOllamaModels(true);
              break;
            case 'choose-ai-local-model':
              var modelInput = document.getElementById('cfgOllamaModel');
              if (modelInput && btn.dataset.model) modelInput.value = btn.dataset.model;
              break;
            case 'toggle-ai-key-visibility':
              var secretInput = document.getElementById('cfgLiteLlmKey');
              if (secretInput) {
                secretInput.type = secretInput.type === 'password' ? 'text' : 'password';
                btn.textContent = secretInput.type === 'password' ? 'Show' : 'Hide';
                btn.setAttribute('aria-pressed', secretInput.type === 'text' ? 'true' : 'false');
              }
              break;
            case 'delete-learned-memory':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.memoryId) {
                window.aetherBridge.deleteMemory(btn.dataset.memoryId).catch(function (err) { showHudToast(String(err)); });
              }
              break;
            case 'clear-learned-memories':
              if (window.aetherBridge && window.aetherBridge.isNative && window.confirm('Delete all saved AI memories?')) {
                window.aetherBridge.clearMemories().catch(function (err) { showHudToast(String(err)); });
              }
              break;
            case 'remove-model-key':
              removeNativeModelKey();
              break;
            case 'add-domain-rule':
              addNativeDomainRule();
              break;
            case 'remove-domain-rule':
              if (btn.dataset.domain) {
                delete nativeRouterConfig.domain_overrides[btn.dataset.domain];
                rerenderAiPreservingDraft();
              }
              break;
            case 'add-credential':
              saveNewNativeCredential();
              break;
            case 'delete-credential':
              deleteNativeCredential(btn.dataset.site, btn.dataset.username);
              break;
            case 'autofill-tab':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.autofillActiveTab(btn.dataset.site || null, btn.dataset.username || null)
                  .then(function (msg) {
                    closeSettingsModal();
                    showHudToast(msg || 'Credentials filled into active tab');
                  })
                  .catch(function (err) {
                    showHudToast('AutoFill: ' + err);
                  });
              } else {
                showHudToast('AutoFill available inside native Windows app');
              }
              break;
            case 'reveal-password':
              revealNativeOrDemoPassword(btn.dataset.site, btn.dataset.username);
              break;
            case 'copy-password':
              copyNativeOrDemoPassword(btn.dataset.site, btn.dataset.username, btn.dataset.secret);
              break;
            case 'toggle-inspector':
              toggleWebInspector();
              break;
            case 'close-inspector':
              toggleWebInspector(false);
              break;
            case 'inspector-tab':
              switchInspectorTab(btn.dataset.itab);
              break;
            case 'run-load-test':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                showHudToast('Running concurrent policy & SQLite WAL load test...');
                window.aetherBridge.runPolicyLoadTest(240, 8).then(function (rep) {
                  if (rep) {
                    appendAgentBubble(
                      'Policy & WAL Load Test',
                      'Completed <strong>' + esc(rep.iterations) + '</strong> checks across <strong>' + esc(rep.concurrency) + '</strong> workers in <strong>' + esc(rep.elapsed_ms) + ' ms</strong> (Allow: ' + esc(rep.allowed_count) + ', Gate: ' + esc(rep.approval_gated_count) + ', Deny: ' + esc(rep.denied_count) + ', Macro roundtrips: ' + esc(rep.macro_roundtrips) + ').'
                    );
                    showHudToast('Load test done in ' + rep.elapsed_ms + ' ms');
                  }
                }).catch(function (err) {
                  showHudToast('Load test failed: ' + err);
                });
              } else {
                showHudToast('Load test runs inside native Windows app');
              }
              break;
            case 'check-updates':
              if (window.aetherBridge && window.aetherBridge.isNative) {
                window.aetherBridge.checkForUpdates().then(function (msg) {
                  showHudToast(msg || 'Update check complete');
                }).catch(function (err) {
                  showHudToast(String(err));
                });
              } else {
                showHudToast('Updater runs inside native Windows app');
              }
              break;
            case 'append-automation-step':
              appendDraftAutomationStep();
              break;
            case 'remove-draft-step':
              if (btn.dataset.stepIdx !== undefined) {
                draftAutomationSteps.splice(Number(btn.dataset.stepIdx), 1);
                renderDraftAutomationSteps();
              }
              break;
            case 'move-draft-step':
              moveDraftAutomationStep(Number(btn.dataset.stepIdx || 0), Number(btn.dataset.delta || 0));
              break;
            case 'edit-automation':
              loadAutomationIntoBuilder(btn.dataset.autoId);
              break;
            case 'create-automation':
              createNativeAutomationFromUi();
              break;
            case 'run-automation':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.autoId) {
                window.aetherBridge.runAutomationNow(btn.dataset.autoId)
                  .then(function () { showHudToast('Automation started'); })
                  .catch(function (err) { showHudToast('Run error: ' + err); });
              }
              break;
            case 'stop-automation':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.autoId) {
                window.aetherBridge.stopAutomationJob(btn.dataset.autoId)
                  .then(function () { showHudToast('Automation stopped'); })
                  .catch(function () {});
              }
              break;
            case 'toggle-automation-enabled':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.autoId) {
                var nextEn = btn.dataset.enabled !== 'true';
                window.aetherBridge.toggleAutomationJob(btn.dataset.autoId, nextEn)
                  .then(function () { showHudToast(nextEn ? 'Schedule enabled' : 'Schedule paused'); })
                  .catch(function () {});
              }
              break;
            case 'delete-automation':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.autoId) {
                window.aetherBridge.deleteAutomationJob(btn.dataset.autoId)
                  .then(function () { showHudToast('Workflow deleted'); })
                  .catch(function () {});
              }
              break;
            case 'replay-macro':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.macroId) {
                var curTab = getActiveTab();
                var curUrl = curTab.history[curTab.historyIndex];
                var curPage = getPageData(curUrl);
                togglePopover('aiChatPopover', true);
                window.aetherBridge.replayWorkflowMacro(btn.dataset.macroId, curPage.url, curPage.title)
                  .then(function () { showHudToast('Replaying cached workflow macro'); })
                  .catch(function (err) { showHudToast('Replay error: ' + err); });
              }
              break;
            case 'delete-macro':
              if (window.aetherBridge && window.aetherBridge.isNative && btn.dataset.macroId) {
                window.aetherBridge.deleteWorkflowMacro(btn.dataset.macroId)
                  .then(function () { showHudToast('Cached macro deleted'); })
                  .catch(function () {});
              }
              break;
          }
        });

        document.addEventListener('mousedown', function (e) {
          var omniItem = e.target.closest('[data-omni-action]');
          if (!omniItem) return;
          e.preventDefault();
          var act = omniItem.dataset.omniAction;
          document.getElementById('omnibarDropdown').classList.add('popover-closed');
          if (act === 'navigate') {
            navigateToUrl(omniItem.dataset.url);
          } else if (act === 'ask-ai') {
            // An omnibar question belongs to the full-page conversation, not a pop-out.
            if (getActiveTab().history[getActiveTab().historyIndex] !== 'browsphere://start') {
              navigateToUrl('browsphere://start');
            }
            newTabChatMode = 'ask';
            submitNewTabChat(omniItem.dataset.query);
          } else if (act === 'settings') {
            openSettingsModal('general');
          }
        });

        document.addEventListener('change', function (e) {
          var el = e.target;
          if (el.dataset.startpageToggle) {
            startPagePrefs[el.dataset.startpageToggle] = el.checked;
            renderActiveTab();
            return;
          }
          if (el.dataset.settingKey) {
            var key = el.dataset.settingKey;
            var val = el.type === 'checkbox' ? el.checked : el.value;
            if (el.dataset.settingType === 'number') val = Number(val);
            updateSetting(key, val);
          } else if (el.dataset.extIndex !== undefined) {
            var idx = Number(el.dataset.extIndex);
            if (browserSettings.extensions[idx]) {
              browserSettings.extensions[idx].enabled = el.checked;
              showHudToast(browserSettings.extensions[idx].name + (el.checked ? ' enabled' : ' disabled'));
            }
          }
        });

        document.getElementById('omnibarForm').addEventListener('submit', function (e) {
          e.preventDefault();
          var val = document.getElementById('omnibarInput').value.trim();
          if (!val) return;
          document.getElementById('omnibarDropdown').classList.add('popover-closed');
          navigateToUrl(val);
        });

        var omnibarInput = document.getElementById('omnibarInput');
        omnibarInput.addEventListener('focus', function () {
          omnibarInput.select();
          renderOmnibarDropdown(omnibarInput.value);
          document.getElementById('omnibarDropdown').classList.remove('popover-closed');
        });
        omnibarInput.addEventListener('blur', function () {
          setTimeout(function () {
            document.getElementById('omnibarDropdown').classList.add('popover-closed');
          }, 160);
        });
        omnibarInput.addEventListener('input', function () {
          renderOmnibarDropdown(omnibarInput.value);
        });

        document.getElementById('sidebarSearchInput').addEventListener('input', renderSidebarSection);
        document.getElementById('findInput').addEventListener('input', function (e) {
          runFindInPage(e.target.value);
        });
        document.getElementById('tabOverviewSearch').addEventListener('input', renderTabOverviewGrid);
        document.addEventListener('submit', function (e) {
          if (e.target && e.target.id === 'newTabChatForm') {
            e.preventDefault();
            submitNewTabChat();
          }
        });

        document.addEventListener('keydown', function (e) {
          if (e.target && e.target.id === 'newTabChatInput' && e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            submitNewTabChat();
          }
        });

        document.addEventListener('input', function (e) {
          if (e.target && e.target.id === 'newTabChatInput') {
            e.target.style.height = 'auto';
            e.target.style.height = Math.min(140, e.target.scrollHeight) + 'px';
          }
        });

        document.getElementById('chatComposerForm').addEventListener('submit', function (e) {
          e.preventDefault();
          var input = document.getElementById('chatPromptInput');
          var text = input.value.trim();
          if (!text) return;
          input.value = '';
          submitChatPrompt(text);
        });

        document.getElementById('inspectorConsoleForm').addEventListener('submit', handleInspectorConsoleSubmit);

        document.addEventListener('input', function (e) {
          if (e.target && e.target.id === 'settingsPasswordSearch') {
            passwordSearchQuery = e.target.value;
            renderSettingsPane();
            var refocused = document.getElementById('settingsPasswordSearch');
            if (refocused) {
              refocused.focus();
              refocused.setSelectionRange(refocused.value.length, refocused.value.length);
            }
          }
        });
      }

      function addStartPageFavorite() {
        var label = 'Research Hub';
        if (!startPageFavorites.some(function (f) { return f.title === label; })) {
          startPageFavorites.push({
            title: label,
            url: 'arxiv-review.org/quantum-2026',
            svg: '<span class="font-sans text-[15px] font-semibold">RH</span>'
          });
          renderActiveTab();
          showHudToast('Added "' + label + '" to Favorites');
        } else {
          showHudToast('Favorite already pinned');
        }
      }

      /* ========================================================================
         4. NAVIGATION, TABS & PAGE RENDERING
         ======================================================================== */
      function getActiveTab() {
        return tabs.find(function (t) { return t.id === activeTabId; }) || tabs[0];
      }

      function getTabConversation(tab) {
        if (!tab || !tab.chatThreadId) return null;
        return newTabChatHistory.find(function (item) { return item.id === tab.chatThreadId; }) || null;
      }

      function getTabDisplayTitle(tab) {
        var conversation = getTabConversation(tab);
        return conversation ? conversation.title :
          (tab.luciaTask && tab.history[tab.historyIndex] === 'about:blank' ? 'Lucia task tab' :
           getPageData(tab.history[tab.historyIndex]).title);
      }

      function getPageData(url) {
        if (url === 'about:blank') return {
          url: url, title: 'Blank tab', group: 'Open Tabs', isArticle: false,
          renderHtml: function () { return ''; }
        };
        if (PRESET_PAGES[url]) return PRESET_PAGES[url];
        var cleanTitle = url.replace(/^https?:\/\//, '').split('/')[0] || 'New Page';
        return {
          url: url,
          title: cleanTitle,
          group: 'Websites',
          isArticle: false,
          readTime: '',
          summaryBullets: [],
          keyMetrics: '',
          critique: '',
          renderHtml: function () {
            return '<main class="max-w-[660px] mx-auto px-6 py-20 text-center space-y-3">' +
              '<h1 class="text-xl font-semibold">Live website view unavailable</h1>' +
              '<p class="text-sm opacity-70">Open Browsphere as a Windows desktop application ' +
              'to browse ' + esc(cleanTitle) + ' using WebView2. This fallback page does not ' +
              'pretend to contain the real website content.</p>' +
              '</main>';
          }
        };
      }

      function refreshOmnibar(force) {
        var omnibar = document.getElementById('omnibarInput');
        if (!omnibar || (!force && document.activeElement === omnibar)) return;
        var tab = getActiveTab();
        var currentUrl = tab.history[tab.historyIndex];
        omnibar.value = window.BrowsphereState
          ? window.BrowsphereState.displayAddress(currentUrl, browserSettings.showFullWebsiteAddress)
          : currentUrl;
      }

      function renderActiveTab(forceAddress) {
        var tab = getActiveTab();
        var currentUrl = tab.history[tab.historyIndex];
        var page = getPageData(currentUrl);

        refreshOmnibar(Boolean(forceAddress));

        document.getElementById('btnHistoryBack').disabled = tab.historyIndex <= 0;
        document.getElementById('btnHistoryForward').disabled = tab.historyIndex >= tab.history.length - 1;

        var readerBtn = document.getElementById('readerModeBtn');
        if (page.isArticle) {
          readerBtn.classList.remove('hidden');
        } else {
          readerBtn.classList.add('hidden');
          if (isReaderMode) toggleReaderMode(false);
        }

        var viewport = document.getElementById('pageViewport');
        var isNativeExternal = Boolean(
          window.aetherBridge &&
          window.aetherBridge.isNative &&
          currentUrl.indexOf('browsphere://') !== 0 &&
          (currentUrl !== 'about:blank' || Boolean(tab.luciaTask))
        );
        viewport.innerHTML = isNativeExternal
          ? '<div class="w-full h-full min-h-full bg-transparent"></div>'
          : page.renderHtml();
        document.getElementById('readingCanvas').scrollTop = 0;
        applyReaderStyles();

        document.getElementById('chatContextPill').textContent = 'Context: ' + page.title;
        document.getElementById('sharePageTitle').textContent = page.title;
        seedChatForPage(page);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.syncAiChatContext(
            page.title,
            currentUrl,
            document.documentElement.getAttribute('data-theme') === 'dark'
          );
        }

        renderSidebarSection();
        renderSeparateTabStrip();
        if (!document.getElementById('webInspectorDrawer').classList.contains('hidden')) {
          renderWebInspector();
        }
      }

      // Keep real link clicks, redirects and address-bar state in step with WebView2.
      function syncCompletedNativeNavigation(detail) {
        if (!detail || !detail.tabId || !detail.url || !window.BrowsphereState) return;
        var tab = tabs.find(function (entry) { return entry.id === detail.tabId; });
        var url = window.BrowsphereState.validAddress(detail.url);
        if (!tab || !url || !/^https?:\/\//i.test(url)) return;
        if (tab.history[tab.historyIndex] === url) return;
        if (isPrivateMode) tab.privateContext = true;
        tab.history = tab.history.slice(0, tab.historyIndex + 1);
        tab.history.push(url);
        tab.historyIndex = tab.history.length - 1;
        if (!isPrivateMode && !tab.privateContext) {
          var page = getPageData(url);
          browsingHistory.unshift({
            title: page.title, url: url,
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
          });
          if (window.aetherBridge && window.aetherBridge.recordCompletedNavigation) {
            window.aetherBridge.recordCompletedNavigation(url, page.title).catch(function () {});
          }
        }
        saveBrowserSession();
        if (tab.id === activeTabId) renderActiveTab(true);
      }

      function navigateToUrl(rawInput) {
        var query = String(rawInput || '').trim();
        if (!query) return;

        if (query.toLowerCase() === 'browsphere://settings' || query.toLowerCase() === 'browsphere://preferences') {
          openSettingsModal('general');
          renderActiveTab();
          return;
        }

        query = window.BrowsphereState.addressOrSearch(query, browserSettings.searchEngine);
        if (!query) {
          showHudToast('Invalid or unsupported web address');
          return;
        }

        var tab = getActiveTab();
        if (!isPrivateMode && tab.privateContext) {
          // Never copy private-page history into a subsequent normal session.
          tab.history = [];
          tab.historyIndex = -1;
          tab.privateContext = false;
        }
        if (isPrivateMode) tab.privateContext = true;
        tab.history = tab.history.slice(0, tab.historyIndex + 1);
        tab.history.push(query);
        tab.historyIndex = tab.history.length - 1;
        saveBrowserSession();

        var page = getPageData(query);
        if (!isPrivateMode) {
          var now = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
          browsingHistory.unshift({ title: page.title, url: query, time: now });
        }

        closeAllPopovers();
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.onNavigate(tab.id, query, page.title);
        }
      }

      function goHistory(delta) {
        var tab = getActiveTab();
        var nextIdx = tab.historyIndex + delta;
        if (nextIdx < 0 || nextIdx >= tab.history.length) return;
        tab.historyIndex = nextIdx;
        if (isPrivateMode) tab.privateContext = true;
        saveBrowserSession();
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var u = tab.history[tab.historyIndex];
          var resolved = u;
          var p = getPageData(u);
          if (resolved.indexOf('http://') === 0 || resolved.indexOf('https://') === 0) {
            window.aetherBridge.onNavigate(tab.id, resolved, p.title);
          } else {
            window.aetherBridge.onSwitchTab(tab.id, resolved);
          }
        }
      }

      function reloadActivePage() {
        var icon = document.getElementById('reloadIcon');
        icon.classList.remove('spin-reload');
        void icon.offsetWidth;
        icon.classList.add('spin-reload');
        var tab = getActiveTab();
        var url = tab.history[tab.historyIndex];
        if (window.aetherBridge && window.aetherBridge.isNative &&
            /^https?:\/\//i.test(url)) {
          window.aetherBridge.reloadTab(tab.id)
            .then(function () { showHudToast('Website reloaded'); })
            .catch(function (err) { showHudToast('Reload failed: ' + String(err)); });
          return;
        }
        renderActiveTab();
        showHudToast('Start page refreshed');
      }

      function createNewTab(url) {
        var targetUrl = url;
        if (!targetUrl) {
          if (browserSettings.newTabsOpenWith === 'Homepage') {
            targetUrl = browserSettings.homepageUrl || 'browsphere://start';
          } else if (browserSettings.newTabsOpenWith === 'Empty Page') {
            targetUrl = 'about:blank';
          } else {
            targetUrl = 'browsphere://start';
          }
        }
        targetUrl = window.BrowsphereState.addressOrSearch(targetUrl, browserSettings.searchEngine) || 'browsphere://start';
        var newId = 'tab-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8);
        tabs.push({
          id: newId,
          group: 'Open Tabs',
          history: [targetUrl],
          historyIndex: 0,
          privateContext: isPrivateMode
        });
        activeTabId = newId;
        saveBrowserSession();
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var resolved = targetUrl;
          if (resolved.indexOf('http://') === 0 || resolved.indexOf('https://') === 0) {
            window.aetherBridge.onNavigate(newId, resolved, getPageData(targetUrl).title);
          } else {
            window.aetherBridge.onSwitchTab(newId, resolved);
          }
        }
      }

      function createLuciaTaskTab(sourceUrl) {
        var taskId = 'tab-lucia-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8);
        var initialUrl = /^https?:\/\//i.test(sourceUrl || '') ? sourceUrl : 'about:blank';
        tabs.push({
          id: taskId, group: 'Lucia', luciaTask: true,
          history: [initialUrl], historyIndex: 0, privateContext: isPrivateMode
        });
        saveBrowserSession();
        renderSidebarSection();
        renderSeparateTabStrip();
        return taskId;
      }

      function markLuciaTaskTabClosed(tabId, source) {
        var messages = {
          user_close: 'Task stopped because its assigned browser tab was closed.',
          private_session_end: 'Lucia stopped because its private browsing session ended.',
          startup_rollback: 'Lucia stopped because task startup rolled back and closed its tab.'
        };
        newTabChatHistory.forEach(function (thread) {
          if (thread.agentTask && thread.agentTask.tabId === tabId) {
            thread.agentTask.tabClosed = true;
            if (thread.agentTask.status === 'Running') {
              thread.agentTask.error = messages[source] || 'Lucia task tab closed.';
            }
          }
        });
      }

      function closeTab(tabId) {
        if (!tabs.some(function (tab) { return tab.id === tabId; })) return;
        markLuciaTaskTabClosed(tabId, 'user_close');
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.onCloseTab(tabId, 'user_close');
        }
        // Never reuse a task tab as the browser's last ordinary start-page tab.
        if (tabs.length === 1 && tabs[0].luciaTask) {
          tabs.push({id:'tab-home-' + Date.now(), group:'Open Tabs',
            history:['browsphere://start'], historyIndex:0, privateContext:isPrivateMode});
        }
        if (tabs.length === 1) {
          var onlyTab = getActiveTab();
          onlyTab.history = ['browsphere://start'];
          onlyTab.historyIndex = 0;
          onlyTab.privateContext = isPrivateMode;
          saveBrowserSession();
          if (window.aetherBridge && window.aetherBridge.isNative) {
            window.aetherBridge.onSwitchTab(onlyTab.id, 'browsphere://start');
          }
          renderActiveTab(true);
          return;
        }
        var idx = tabs.findIndex(function (t) { return t.id === tabId; });
        tabs = tabs.filter(function (t) { return t.id !== tabId; });
        if (activeTabId === tabId) {
          activeTabId = tabs[Math.max(0, idx - 1)].id;
        }
        saveBrowserSession();
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var nextTab = getActiveTab();
          var nextUrl = nextTab.history[nextTab.historyIndex];
          var resolved = nextUrl;
          window.aetherBridge.onSwitchTab(nextTab.id, resolved);
        }
        if (!document.getElementById('tabOverviewModal').classList.contains('hidden')) {
          renderTabOverviewGrid();
        }
      }

      function switchToTab(tabId) {
        if (!tabs.some(function (t) { return t.id === tabId; })) return;
        activeTabId = tabId;
        saveBrowserSession();
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var t = getActiveTab();
          var u = t.history[t.historyIndex];
          window.aetherBridge.onSwitchTab(t.id, u);
        }
      }

      /* ========================================================================
         5. LEFT SIDEBAR (TABS, BOOKMARKS, READING LIST, HISTORY)
         ======================================================================== */
      function switchSidebarSection(section) {
        sidebarSection = section;
        ['tabs', 'bookmarks', 'reading', 'history'].forEach(function (key) {
          var btn = document.getElementById('seg-' + key);
          if (!btn) return;
          btn.className = key === section
            ? 'py-1 rounded-md bg-white dark:bg-zinc-700 shadow-2xs transition'
            : 'py-1 rounded-md opacity-65 hover:opacity-100 transition';
        });
        renderSidebarSection();
      }

      function renderSidebarSection() {
        var container = document.getElementById('sidebarDynamicBody');
        var filter = (document.getElementById('sidebarSearchInput').value || '').trim().toLowerCase();

        if (sidebarSection === 'tabs') {
          var groupSet = {};
          tabs.forEach(function (t) { groupSet[t.group || 'Open Tabs'] = true; });
          var groups = Object.keys(groupSet);
          var groupsHtml = groups.map(function (groupName) {
            var groupTabs = tabs.filter(function (t) {
              var p = getPageData(t.history[t.historyIndex]);
              return t.group === groupName && (getTabDisplayTitle(t).toLowerCase().indexOf(filter) !== -1 || p.url.toLowerCase().indexOf(filter) !== -1);
            });
            if (groupTabs.length === 0 && filter) return '';

            var itemsHtml = groupTabs.map(function (t) {
              var p = getPageData(t.history[t.historyIndex]);
              var isActive = t.id === activeTabId;
              var itemClasses = isActive
                ? 'bg-white dark:bg-zinc-700/90 font-semibold shadow-2xs ring-1 ring-black/5 dark:ring-white/10'
                : 'opacity-75 hover:opacity-100 hover:bg-black/5 dark:hover:bg-white/10 font-medium';
              return (
                '<li>' +
                  '<div data-action="switch-tab" data-tab-id="' + t.id + '" class="group flex items-center justify-between gap-1.5 px-2 py-1.5 rounded-lg text-[12px] cursor-pointer transition ' + itemClasses + '">' +
                    '<span class="truncate">' + esc(getTabDisplayTitle(t)) + '</span>' +
                    '<button type="button" data-action="close-tab" data-tab-id="' + t.id + '" title="Close Tab" class="opacity-0 group-hover:opacity-70 hover:!opacity-100 px-1 rounded text-[11px]">✕</button>' +
                  '</div>' +
                '</li>'
              );
            }).join('');

            return (
              '<div class="space-y-1">' +
                '<div class="flex items-center justify-between px-1.5 text-[10.5px] font-semibold opacity-55">' +
                  '<span>' + groupName + '</span>' +
                  '<span>' + groupTabs.length + '</span>' +
                '</div>' +
                '<ul class="space-y-0.5">' + itemsHtml + '</ul>' +
              '</div>'
            );
          }).join('');

          container.innerHTML = groupsHtml +
            '<button type="button" data-action="new-tab" class="w-full mt-2 py-1.5 px-2.5 rounded-lg border border-dashed border-black/15 dark:border-white/15 text-[11.5px] font-medium opacity-75 hover:opacity-100 hover:border-[#007AFF] hover:text-[#007AFF] transition">+ New Tab</button>';

        } else if (sidebarSection === 'bookmarks') {
          var folderMap = {};
          bookmarks.forEach(function (b) { folderMap[b.folder] = true; });
          var folders = Object.keys(folderMap);

          var foldersHtml = folders.map(function (folder) {
            var items = bookmarks.filter(function (b) {
              return b.folder === folder && b.title.toLowerCase().indexOf(filter) !== -1;
            });
            if (!items.length) return '';
            var rows = items.map(function (b) {
              return '<button type="button" data-action="navigate" data-url="' + esc(b.url) + '" class="w-full text-left px-2 py-1.5 rounded-lg text-[12px] hover:bg-black/5 dark:hover:bg-white/10 truncate block">🔖 ' + esc(b.title) + '</button>';
            }).join('');
            return '<div class="space-y-1"><div class="px-1.5 text-[10.5px] font-semibold opacity-55">' + esc(folder) + '</div>' + rows + '</div>';
          }).join('');

          container.innerHTML =
            '<button type="button" data-action="bookmark-current" class="w-full py-1.5 px-2.5 rounded-lg bg-[#007AFF]/12 text-[#007AFF] text-[11.5px] font-semibold hover:bg-[#007AFF]/20 transition">+ Bookmark Current Page</button>' +
            foldersHtml;

        } else if (sidebarSection === 'reading') {
          var items = readingList.filter(function (r) { return r.title.toLowerCase().indexOf(filter) !== -1; });
          var rows = items.map(function (item, idx) {
            var dot = item.unread ? '<span class="w-2 h-2 rounded-full bg-[#007AFF] shrink-0"></span>' : '';
            return (
              '<button type="button" data-action="open-reading" data-index="' + idx + '" class="w-full text-left p-2 rounded-lg bg-black/[0.03] dark:bg-white/[0.05] hover:bg-black/[0.06] dark:hover:bg-white/[0.09] transition block">' +
                '<div class="flex items-center gap-1.5 text-[12px] font-semibold">' + dot + '<span class="truncate">' + esc(item.title) + '</span></div>' +
                '<div class="text-[10.5px] opacity-55 mt-0.5 flex justify-between"><span>' + esc(item.readTime) + '</span><span>Saved Offline</span></div>' +
              '</button>'
            );
          }).join('');

          container.innerHTML =
            '<button type="button" data-action="reading-add" class="w-full py-1.5 px-2.5 rounded-lg bg-[#007AFF]/12 text-[#007AFF] text-[11.5px] font-semibold hover:bg-[#007AFF]/20 transition">+ Save Current to Reading List</button>' +
            '<div class="space-y-1.5 mt-2">' + rows + '</div>';

        } else if (sidebarSection === 'history') {
          var histItems = browsingHistory.filter(function (h) {
            return h.title.toLowerCase().indexOf(filter) !== -1 || h.url.toLowerCase().indexOf(filter) !== -1;
          });
          var histRows = histItems.length
            ? histItems.map(function (h) {
                return (
                  '<button type="button" data-action="navigate" data-url="' + esc(h.url) + '" class="w-full text-left px-2 py-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10 transition block">' +
                    '<div class="text-[12px] font-medium truncate">' + esc(h.title) + '</div>' +
                    '<div class="text-[10px] opacity-50 flex justify-between"><span class="truncate max-w-[155px]">' + esc(h.url) + '</span><span>' + esc(h.time) + '</span></div>' +
                  '</button>'
                );
              }).join('')
            : '<p class="text-xs opacity-50 px-2 py-4 text-center">No history items</p>';

          container.innerHTML =
            '<div class="flex items-center justify-between px-1">' +
              '<span class="text-[10.5px] font-semibold opacity-55">Today</span>' +
              '<button type="button" data-action="clear-history" class="text-[10.5px] text-rose-500 hover:underline">Clear History</button>' +
            '</div>' +
            '<div class="space-y-1 mt-1">' + histRows + '</div>';
        }
      }

      function bookmarkCurrentPage() {
        var tab = getActiveTab();
        var url = tab.history[tab.historyIndex];
        var page = getPageData(url);
        if (!bookmarks.some(function (b) { return b.url === url; })) {
          bookmarks.unshift({ folder: 'Favorites', title: page.title, url: url });
        }
        saveBrowserLibrary();
        closeAllPopovers();
        renderSidebarSection();
        showHudToast('Bookmarked "' + page.title + '"');
      }

      function addToReadingList() {
        var tab = getActiveTab();
        var url = tab.history[tab.historyIndex];
        var page = getPageData(url);
        if (!readingList.some(function (r) { return r.url === url; })) {
          readingList.unshift({ title: page.title, url: url, readTime: page.readTime || '5 min', unread: true });
        }
        saveBrowserLibrary();
        closeAllPopovers();
        renderSidebarSection();
        showHudToast('Saved "' + page.title + '" to Reading List');
      }

      function openReadingItem(idx) {
        var item = readingList[idx];
        if (!item) return;
        item.unread = false;
        saveBrowserLibrary();
        navigateToUrl(item.url);
      }

      /* ========================================================================
         6. OMNIBAR AUTOCOMPLETE DROPDOWN
         ======================================================================== */
      function renderOmnibarDropdown(query) {
        var q = String(query || '').trim();
        var lower = q.toLowerCase();
        var content = document.getElementById('omnibarDropdownContent');

        var matchingPages = startPageFavorites.filter(function (p) {
          return p.title.toLowerCase().indexOf(lower) !== -1 || p.url.toLowerCase().indexOf(lower) !== -1;
        });

        var querySection = '';
        if (q) {
          querySection =
            '<div class="space-y-1">' +
              '<button type="button" data-omni-action="navigate" data-url="' + esc(q) + '" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10 font-medium transition text-left">' +
                '<span class="truncate">🔍 Search <strong>' + esc(browserSettings.searchEngine) + '</strong> for "' + esc(q) + '"</span>' +
                '<span class="text-[10px] font-mono opacity-50 shrink-0">↵</span>' +
              '</button>' +
              '<button type="button" data-omni-action="ask-ai" data-query="' + esc(q) + '" class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg bg-[#007AFF]/10 text-[#007AFF] font-medium hover:bg-[#007AFF]/18 transition text-left">' +
                '<span class="truncate">✦ Ask ' + esc(browserSettings.aiEngine.split(' ')[0]) + ' AI about "<strong>' + esc(q) + '</strong>"</span>' +
                '<span class="text-[10px] font-mono shrink-0">AI Chat ↵</span>' +
              '</button>' +
            '</div>';
        }

        var hitsHtml = matchingPages.map(function (p) {
          return (
            '<button type="button" data-omni-action="navigate" data-url="' + esc(p.url) + '" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10 text-left transition">' +
              '<span class="font-medium truncate">' + esc(p.title) + '</span>' +
              '<span class="text-[11px] font-mono opacity-50 truncate max-w-[220px]">' + esc(p.url) + '</span>' +
            '</button>'
          );
        }).join('');

        content.innerHTML = querySection +
          '<div class="text-[10.5px] font-semibold uppercase tracking-wider opacity-50 px-2 pt-1">Top Hits &amp; Favorites</div>' +
          '<div class="space-y-0.5">' +
            hitsHtml +
            '<button type="button" data-omni-action="settings" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10 text-left transition">' +
              '<span class="font-medium">⚙️ Browsphere Settings &amp; Preferences</span>' +
              '<span class="text-[11px] font-mono opacity-50">browsphere://settings</span>' +
            '</button>' +
          '</div>';
      }

      /* ========================================================================
         7. ANCHORED TOP-RIGHT AI QUICK CHAT POPOVER
         ======================================================================== */
      function seedChatForPage(page) {
        var stream = document.getElementById('chatMessagesStream');
        stream.innerHTML =
          '<div class="text-center py-8 opacity-70 space-y-1">' +
            '<p class="text-xs font-semibold">Lucia · ' + esc(page.title) + '</p>' +
            '<p class="text-[11px] opacity-70">Ask a question or assign a policy-gated browsing task.</p>' +
          '</div>';
      }

      function submitChatPrompt(promptText) {
        if (getActiveTab().history[getActiveTab().historyIndex] === 'browsphere://start') {
          submitNewTabChat(promptText);
          return;
        }
        var stream = document.getElementById('chatMessagesStream');
        var tab = getActiveTab();
        var page = getPageData(tab.history[tab.historyIndex]);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var isDark = document.documentElement.getAttribute('data-theme') === 'dark';
          window.aetherBridge.toggleAiChatWindow(true, page.title, page.url, isDark, promptText);
          return;
        }

        var userEl = document.createElement('div');
        userEl.className = 'flex justify-end';
        userEl.innerHTML = '<div class="bg-[#007AFF] text-white px-3 py-2 rounded-2xl rounded-br-sm max-w-[85%] text-[12px]">' + esc(promptText) + '</div>';
        stream.appendChild(userEl);
        stream.scrollTop = stream.scrollHeight;

        var aiEl = document.createElement('div');
        aiEl.className = 'flex items-start gap-2';
        aiEl.innerHTML =
          '<span class="w-5 h-5 rounded-full bg-gradient-to-tr from-[#007AFF] to-[#AF52DE] text-white flex items-center justify-center shrink-0 mt-0.5 text-[9px] font-bold">AI</span>' +
          '<div class="ai-msg-target bg-black/[0.04] dark:bg-white/[0.07] px-3 py-2.5 rounded-2xl rounded-tl-sm text-[12px] max-w-[88%]">' +
            '<div class="flex items-center gap-1 py-1">' +
              '<span class="typing-dot w-1.5 h-1.5 rounded-full bg-current"></span>' +
              '<span class="typing-dot w-1.5 h-1.5 rounded-full bg-current"></span>' +
              '<span class="typing-dot w-1.5 h-1.5 rounded-full bg-current"></span>' +
            '</div>' +
          '</div>';
        stream.appendChild(aiEl);
        stream.scrollTop = stream.scrollHeight;

        var lower = promptText.toLowerCase();
        var tonePrefix = browserSettings.aiTone === 'Academic'
          ? '[' + browserSettings.aiEngine + ' • Academic Synthesis]'
          : browserSettings.aiTone === 'Executive'
            ? '[' + browserSettings.aiEngine + ' • Executive Brief]'
            : '[' + browserSettings.aiEngine + ']';
        var replyHtml = '';

        if (lower.indexOf('3 bullets') !== -1 || lower.indexOf('summarize') !== -1) {
          var bulletsHtml = page.summaryBullets.map(function (b) { return '<li>' + b + '</li>'; }).join('');
          replyHtml =
            '<div class="text-[10px] font-mono opacity-55 mb-1">' + esc(tonePrefix) + '</div>' +
            '<p class="font-semibold mb-1">Key Takeaways (' + esc(page.title) + '):</p>' +
            '<ul class="list-disc pl-4 space-y-1 opacity-90">' + bulletsHtml + '</ul>';
        } else if (lower.indexOf('data') !== -1 || lower.indexOf('metric') !== -1 || lower.indexOf('number') !== -1) {
          replyHtml =
            '<div class="text-[10px] font-mono opacity-55 mb-1">' + esc(tonePrefix) + '</div>' +
            '<p class="font-semibold mb-1">Extracted Key Metrics:</p>' +
            '<div class="font-mono text-[11px] p-2 rounded-lg bg-black/5 dark:bg-white/10 whitespace-pre-line">' + esc(page.keyMetrics) + '</div>';
        } else if (lower.indexOf('fact') !== -1 || lower.indexOf('critique') !== -1) {
          replyHtml =
            '<div class="text-[10px] font-mono opacity-55 mb-1">' + esc(tonePrefix) + '</div>' +
            '<p class="font-semibold mb-1">Critical Assessment:</p>' +
            '<p class="opacity-90">' + esc(page.critique) + '</p>';
        } else if (lower.indexOf('explain:') === 0) {
          var quote = promptText.replace(/^explain:\s*/i, '');
          replyHtml =
            '<div class="text-[10px] font-mono opacity-55 mb-1">' + esc(tonePrefix) + '</div>' +
            '<p class="font-semibold text-[#007AFF] mb-1">Passage Explanation:</p>' +
            '<p class="opacity-90">In <strong>' + esc(page.title) + '</strong>, <em>' + esc(quote) + '</em> highlights the core architectural breakthrough that eliminates traditional hardware latency bottlenecks.</p>';
        } else {
          replyHtml =
            '<div class="text-[10px] font-mono opacity-55 mb-1">' + esc(tonePrefix) + '</div>' +
            '<p class="opacity-90">Based on <strong>' + esc(page.title) + '</strong>, this approach reduces latency overhead while keeping execution fast and verifiable. Want me to extract all quantitative figures or compare with another tab?</p>';
        }

        var matchedSrcs = getMatchingSourcesForPrompt(promptText, page.url);
        var srcStripHtml =
          '<div class="mt-2 pt-2 border-t border-black/[0.07] dark:border-white/[0.1]">' +
            '<div class="text-[10px] opacity-60 mb-1">Sources (hover thumbnail to verify)</div>' +
            '<div class="flex flex-wrap gap-1">' + buildSourceChipsHtml(matchedSrcs) + '</div>' +
          '</div>';

        setTimeout(function () {
          var target = aiEl.querySelector('.ai-msg-target');
          if (target) {
            target.innerHTML = replyHtml + srcStripHtml;
            stream.scrollTop = stream.scrollHeight;
          }
        }, 320);
      }

      function clearChatMessages() {
        var tab = getActiveTab();
        var page = getPageData(tab.history[tab.historyIndex]);
        document.getElementById('chatMessagesStream').innerHTML =
          '<div class="text-center py-6 opacity-75">' +
            '<p class="text-xs font-semibold">New Thread • ' + esc(page.title) + '</p>' +
            '<p class="text-[11px] opacity-65 mt-1">Ask any question or highlight text on the page.</p>' +
          '</div>';
      }

      function setVoiceUiActive(active) {
        isVoiceDictating = active;
        var mic = document.getElementById('voiceMicSvg');
        var bars = document.getElementById('voiceWaveBars');
        var input = document.getElementById('chatPromptInput');
        if (mic) mic.classList.toggle('hidden', active);
        if (bars) {
          bars.classList.toggle('hidden', !active);
          bars.classList.toggle('flex', active);
        }
        if (input) {
          input.placeholder = active ? 'Listening...' : 'Ask anything about this page...';
        }
      }

      function toggleVoiceDictation() {
        if (isVoiceDictating) {
          setVoiceUiActive(false);
          return;
        }

        var input = document.getElementById('chatPromptInput');
        setVoiceUiActive(true);

        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.listenSpeechOnce(8).then(function (transcript) {
            setVoiceUiActive(false);
            if (transcript && input) {
              input.value = transcript;
              input.focus();
              showHudToast('Transcribed voice prompt');
            } else {
              showHudToast('No speech detected');
            }
          }).catch(function () {
            setVoiceUiActive(false);
            showHudToast('Microphone recognition unavailable');
          });
          return;
        }

        var SR = window.SpeechRecognition || window.webkitSpeechRecognition;
        if (SR) {
          try {
            var rec = new SR();
            rec.lang = 'en-US';
            rec.interimResults = false;
            rec.onresult = function (ev) {
              var txt = (ev && ev.results && ev.results[0] && ev.results[0][0]) ? ev.results[0][0].transcript : '';
              if (txt && input) {
                input.value = txt;
                input.focus();
              }
            };
            rec.onend = function () { setVoiceUiActive(false); };
            rec.onerror = function () { setVoiceUiActive(false); };
            rec.start();
            return;
          } catch (_) {}
        }

        setTimeout(function () {
          setVoiceUiActive(false);
          showHudToast('No speech recognizer available');
        }, 1200);
      }

      /* ========================================================================
         8. INLINE TEXT SELECTION & HIGHLIGHTS
         ======================================================================== */
      function setupSelectionListener() {
        var canvas = document.getElementById('readingCanvas');
        var pill = document.getElementById('selectionTooltip');

        canvas.addEventListener('mouseup', function () {
          if (!browserSettings.inlineSelectionPopover) {
            pill.style.display = 'none';
            return;
          }
          var sel = window.getSelection();
          var text = sel ? sel.toString().trim() : '';
          if (text.length > 6) {
            selectedTextCache = text;
            var rect = sel.getRangeAt(0).getBoundingClientRect();
            pill.style.display = 'flex';
            pill.style.top = Math.max(68, rect.top - 44) + 'px';
            pill.style.left = Math.min(window.innerWidth - 250, Math.max(20, rect.left + rect.width / 2 - 115)) + 'px';
          } else {
            pill.style.display = 'none';
          }
        });

        document.addEventListener('mousedown', function (e) {
          if (!pill.contains(e.target)) pill.style.display = 'none';
        });
      }

      function clickSmartHighlight(el) {
        if (!browserSettings.inlineSelectionPopover) return;
        selectedTextCache = el.textContent.trim();
        var rect = el.getBoundingClientRect();
        var pill = document.getElementById('selectionTooltip');
        pill.style.display = 'flex';
        pill.style.top = Math.max(68, rect.top - 44) + 'px';
        pill.style.left = Math.min(window.innerWidth - 250, Math.max(20, rect.left + 16)) + 'px';
      }

      function handleInlineAiAction(type) {
        document.getElementById('selectionTooltip').style.display = 'none';
        if (!selectedTextCache) return;
        var snippet = selectedTextCache.length > 85 ? selectedTextCache.slice(0, 82) + '...' : selectedTextCache;

        togglePopover('aiChatPopover', true);
        if (type === 'explain') {
          submitChatPrompt('Explain: "' + snippet + '"');
        } else if (type === 'summarize') {
          submitChatPrompt('Summarize: "' + snippet + '"');
        } else {
          var input = document.getElementById('chatPromptInput');
          input.value = 'Regarding "' + snippet + '" — ';
          input.focus();
        }
      }

      /* ========================================================================
         9. READER MODE, IN-PAGE FIND, DOWNLOADS, SHARE & TAB OVERVIEW
         ======================================================================== */
      function toggleReaderMode(force) {
        isReaderMode = typeof force === 'boolean' ? force : !isReaderMode;
        var bar = document.getElementById('readerToolbar');
        var btn = document.getElementById('readerModeBtn');

        if (isReaderMode) {
          bar.classList.remove('hidden');
          bar.classList.add('flex');
          btn.classList.add('bg-[#007AFF]', 'text-white');
        } else {
          bar.classList.add('hidden');
          bar.classList.remove('flex');
          btn.classList.remove('bg-[#007AFF]', 'text-white');
          var canvas = document.getElementById('readingCanvas');
          canvas.classList.remove('reader-theme-white', 'reader-theme-sepia', 'reader-theme-gray', 'reader-theme-night');
          if (isTtsPlaying) togglePageTts();
        }
        applyReaderStyles();
      }

      function adjustReaderFont(delta) {
        readerFontSize = Math.min(24, Math.max(14, readerFontSize + delta));
        applyReaderStyles();
      }

      function setReaderTheme(theme) {
        readerTheme = theme;
        applyReaderStyles();
      }

      function applyReaderStyles() {
        var canvas = document.getElementById('readingCanvas');
        canvas.classList.remove('reader-theme-white', 'reader-theme-sepia', 'reader-theme-gray', 'reader-theme-night');
        if (isReaderMode) {
          canvas.classList.add('reader-theme-' + readerTheme);
        }
        document.querySelectorAll('.reader-body').forEach(function (el) {
          el.style.fontSize = readerFontSize + 'px';
        });
      }

      function extractReadableArticleText() {
        var bodyEl = document.querySelector('#pageViewport .reader-body') || document.getElementById('pageViewport');
        var raw = bodyEl ? (bodyEl.innerText || bodyEl.textContent || '') : '';
        return raw.replace(/\s+/g, ' ').trim().slice(0, 3500);
      }

      function setTtsUiPlaying(playing) {
        isTtsPlaying = playing;
        var label = document.getElementById('ttsLabel');
        var icon = document.getElementById('ttsIcon');
        if (label) label.textContent = playing ? 'Stop Listening' : 'Listen to Page';
        if (icon) {
          icon.innerHTML = playing
            ? '<rect x="6" y="5" width="4" height="14"/><rect x="14" y="5" width="4" height="14"/>'
            : '<path d="M8 5v14l11-7z"/>';
        }
      }

      function togglePageTts() {
        if (isTtsPlaying) {
          setTtsUiPlaying(false);
          if (window.aetherBridge && window.aetherBridge.isNative) {
            window.aetherBridge.stopSpeaking();
          }
          if (typeof window.speechSynthesis !== 'undefined') {
            try { window.speechSynthesis.cancel(); } catch (_) {}
          }
          showHudToast('Stopped reading aloud');
          return;
        }

        var text = extractReadableArticleText();
        if (!text) {
          showHudToast('No readable article text found');
          return;
        }

        setTtsUiPlaying(true);
        showHudToast('Reading article aloud...');

        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.speakText(text, 0).then(function () {
            setTtsUiPlaying(false);
          }).catch(function () {
            setTtsUiPlaying(false);
          });
          return;
        }

        if (typeof window.speechSynthesis !== 'undefined' && typeof SpeechSynthesisUtterance !== 'undefined') {
          try {
            window.speechSynthesis.cancel();
            var utter = new SpeechSynthesisUtterance(text);
            utter.onend = function () { setTtsUiPlaying(false); };
            utter.onerror = function () { setTtsUiPlaying(false); };
            window.speechSynthesis.speak(utter);
          } catch (_) {
            setTtsUiPlaying(false);
          }
        } else {
          setTimeout(function () { setTtsUiPlaying(false); }, 2500);
        }
      }

      function toggleFindBar(force) {
        var bar = document.getElementById('findInPageBar');
        var show = typeof force === 'boolean' ? force : bar.classList.contains('hidden');
        if (show) {
          bar.classList.remove('hidden');
          bar.classList.add('flex');
          document.getElementById('findInput').focus();
        } else {
          bar.classList.add('hidden');
          bar.classList.remove('flex');
          clearFindHighlights();
        }
      }

      function clearFindHighlights() {
        document.querySelectorAll('mark.find-match').forEach(function (m) {
          var parent = m.parentNode;
          parent.replaceChild(document.createTextNode(m.textContent), m);
          parent.normalize();
        });
        findMatches = [];
        activeFindIndex = -1;
        document.getElementById('findMatchCounter').textContent = '0/0';
      }

      function runFindInPage(query) {
        clearFindHighlights();
        var q = String(query || '').trim();
        if (!q) return;

        var root = document.getElementById('pageViewport');
        var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
        var nodes = [];
        var n;
        while ((n = walker.nextNode())) {
          if (n.nodeValue.toLowerCase().indexOf(q.toLowerCase()) !== -1) nodes.push(n);
        }

        var escapedRegex = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        var regex = new RegExp('(' + escapedRegex + ')', 'gi');

        nodes.forEach(function (textNode) {
          var parts = textNode.nodeValue.split(regex);
          if (parts.length > 1) {
            var frag = document.createDocumentFragment();
            parts.forEach(function (part) {
              if (part.toLowerCase() === q.toLowerCase()) {
                var mark = document.createElement('mark');
                mark.className = 'find-match';
                mark.textContent = part;
                frag.appendChild(mark);
                findMatches.push(mark);
              } else {
                frag.appendChild(document.createTextNode(part));
              }
            });
            textNode.parentNode.replaceChild(frag, textNode);
          }
        });

        if (findMatches.length > 0) {
          activeFindIndex = 0;
          highlightActiveFindMatch();
        }
      }

      function stepFindMatch(delta) {
        if (!findMatches.length) return;
        activeFindIndex = (activeFindIndex + delta + findMatches.length) % findMatches.length;
        highlightActiveFindMatch();
      }

      function highlightActiveFindMatch() {
        findMatches.forEach(function (m, i) {
          m.classList.toggle('active-match', i === activeFindIndex);
        });
        var current = findMatches[activeFindIndex];
        if (current) current.scrollIntoView({ behavior: 'smooth', block: 'center' });
        document.getElementById('findMatchCounter').textContent = (activeFindIndex + 1) + '/' + findMatches.length;
      }

      function formatBytesHuman(n) {
        if (!n) return '0 B';
        var units = ['B', 'KB', 'MB', 'GB'];
        var idx = 0, val = Number(n);
        while (val >= 1024 && idx < units.length - 1) { val /= 1024; idx += 1; }
        return val.toFixed(idx === 0 ? 0 : 1) + ' ' + units[idx];
      }

      function renderDownloadsList() {
        var container = document.getElementById('downloadsListContainer');
        if (!downloads.length) {
          container.innerHTML = '<p class="text-center opacity-50 py-4 text-[11px]">No recent downloads</p>';
          document.getElementById('downloadBadgeDot').classList.add('hidden');
          return;
        }
        document.getElementById('downloadBadgeDot').classList.remove('hidden');
        container.innerHTML = downloads.map(function (d) {
          var name = d.filename || d.name || 'download.bin';
          var status = d.status || 'completed';
          var sizeLabel = d.bytes_downloaded !== undefined
            ? (formatBytesHuman(d.bytes_downloaded) + (d.total_bytes ? ' / ' + formatBytesHuman(d.total_bytes) : ''))
            : (d.size || '');
          var pct = d.total_bytes
            ? Math.min(100, Math.round((d.bytes_downloaded / d.total_bytes) * 100))
            : (status === 'completed' ? 100 : status === 'downloading' ? 35 : 0);
          var cancelBtn = status === 'downloading' && d.id
            ? '<button type="button" data-action="cancel-download" data-dl-id="' + esc(d.id) + '" class="px-1.5 py-0.5 rounded bg-rose-500/15 text-rose-500 text-[10px] font-medium shrink-0">Cancel</button>'
            : '<span class="text-[#007AFF] text-xs shrink-0">' + (status === 'completed' ? '✓' : '↓') + '</span>';

          return (
            '<div class="p-2 rounded-lg bg-black/[0.04] dark:bg-white/[0.06] space-y-1.5" title="' + esc(d.saved_path || d.url || '') + '">' +
              '<div class="flex items-center justify-between gap-2">' +
                '<div class="min-w-0">' +
                  '<div class="font-medium truncate text-[11.5px]">' + esc(name) + '</div>' +
                  '<div class="text-[10px] opacity-55">' + esc(sizeLabel) + ' • ' + esc(status) + '</div>' +
                '</div>' +
                cancelBtn +
              '</div>' +
              (status === 'downloading'
                ? '<div class="h-1 w-full bg-black/10 dark:bg-white/10 rounded-full overflow-hidden"><div class="h-full bg-[#007AFF]" style="width:' + pct + '%"></div></div>'
                : '') +
            '</div>'
          );
        }).join('');
      }

      function triggerSimulatedDownload() {
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var tab = getActiveTab();
          var rawUrl = tab.history[tab.historyIndex];
          var targetUrl = (rawUrl.indexOf('http://') === 0 || rawUrl.indexOf('https://') === 0)
            ? rawUrl
            : (rawUrl.indexOf('browsphere://') === 0 ? 'https://example.org/' : 'https://' + rawUrl);
          window.aetherBridge.startDownload(targetUrl, 'page-snapshot.html')
            .then(function () {
              hydrateDownloadsFromNative();
              showHudToast('Streaming download started');
            })
            .catch(function (err) {
              showHudToast('Download error: ' + err);
            });
          return;
        }
        downloads.unshift({
          name: 'Browsphere_AI_Snapshot_' + Math.floor(Math.random() * 90 + 10) + '.pdf',
          size: '2.4 MB',
          status: 'Completed'
        });
        renderDownloadsList();
        showHudToast('Download completed');
      }

      function copyTextToClipboard(text) {
        var ta = document.createElement('textarea');
        ta.value = text;
        document.body.appendChild(ta);
        ta.select();
        try { document.execCommand('copy'); } catch (_) {}
        document.body.removeChild(ta);
      }

      function copyActivePageLink() {
        var tab = getActiveTab();
        copyTextToClipboard('https://' + tab.history[tab.historyIndex]);
        closeAllPopovers();
        showHudToast('Link copied to clipboard');
      }

      function copySnippet(btn) {
        var code = btn.nextElementSibling ? btn.nextElementSibling.textContent : '';
        copyTextToClipboard(code);
        btn.textContent = 'Copied!';
        setTimeout(function () { btn.textContent = 'Copy'; }, 1500);
      }

      function toggleTabOverview(force) {
        var modal = document.getElementById('tabOverviewModal');
        var show = typeof force === 'boolean' ? force : modal.classList.contains('hidden');
        if (show) {
          closeAllPopovers();
          modal.classList.remove('hidden');
          renderTabOverviewGrid();
        } else {
          modal.classList.add('hidden');
        }
      }

      function renderTabOverviewGrid() {
        var filter = (document.getElementById('tabOverviewSearch').value || '').toLowerCase();
        var grid = document.getElementById('tabOverviewGrid');
        var filtered = tabs.filter(function (t) {
          return getTabDisplayTitle(t).toLowerCase().indexOf(filter) !== -1;
        });
        document.getElementById('tabOverviewCount').textContent = filtered.length + ' Open Tabs';

        grid.innerHTML = filtered.map(function (t) {
          var page = getPageData(t.history[t.historyIndex]);
          var isCurrent = t.id === activeTabId;
          var borderClass = isCurrent ? 'border-[#007AFF]' : 'border-white/20';
          return (
            '<div data-action="overview-switch-tab" data-tab-id="' + t.id + '" class="group rounded-2xl overflow-hidden bg-white dark:bg-zinc-900 text-slate-900 dark:text-zinc-100 border-2 ' + borderClass + ' shadow-xl cursor-pointer transition hover:scale-[1.02]">' +
              '<div class="px-3 py-2 bg-black/5 dark:bg-white/10 flex items-center justify-between text-xs">' +
                '<span class="font-semibold truncate">' + esc(getTabDisplayTitle(t)) + '</span>' +
                '<button type="button" data-action="close-tab" data-tab-id="' + t.id + '" class="px-1.5 rounded hover:bg-black/15">✕</button>' +
              '</div>' +
              '<div class="p-4 h-36 overflow-hidden text-[11px] opacity-75 space-y-2">' +
                '<div class="font-mono text-[10px] text-[#007AFF] truncate">' + esc(page.url) + '</div>' +
                '<p class="line-clamp-4 leading-relaxed">' + (getTabConversation(t) ? 'Lucia conversation with ' + getTabConversation(t).messages.length + ' messages' : page.summaryBullets[0].replace(/<[^>]+>/g, '')) + '</p>' +
              '</div>' +
            '</div>'
          );
        }).join('');
      }

      /* ========================================================================
         10. POPOVER MANAGER, THEME, PRIVATE MODE & SHORTCUTS
         ======================================================================== */
      var POPOVER_IDS = ['aiChatPopover', 'automationPopover', 'privacyPopover', 'downloadsPopover', 'sharePopover'];

      function togglePopover(id, forceOpen) {
        if (id === 'aiChatPopover' && window.aetherBridge && window.aetherBridge.isNative) {
          var curTab = getActiveTab();
          var curUrl = curTab.history[curTab.historyIndex];
          var curPage = getPageData(curUrl);
          var isDark = document.documentElement.getAttribute('data-theme') === 'dark';
          closeAllPopovers();
          window.aetherBridge.toggleAiChatWindow(forceOpen, curPage.title, curUrl, isDark, null);
          return;
        }
        var target = document.getElementById(id);
        if (!target) return;
        var isCurrentlyClosed = target.classList.contains('popover-closed');
        var shouldOpen = typeof forceOpen === 'boolean' ? forceOpen : isCurrentlyClosed;

        closeAllPopovers();
        if (shouldOpen) {
          target.classList.remove('popover-closed');
        }
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.syncBounds();
          setTimeout(function () { window.aetherBridge.syncBounds(); }, 180);
        }
      }

      function closeAllPopovers() {
        POPOVER_IDS.forEach(function (pid) {
          var el = document.getElementById(pid);
          if (el) el.classList.add('popover-closed');
        });
      }

      function toggleSidebar() {
        document.getElementById('leftSidebar').classList.toggle('collapsed');
      }

      function toggleThemeMode() {
        var html = document.documentElement;
        var nextIsDark = html.getAttribute('data-theme') !== 'dark';
        html.setAttribute('data-theme', nextIsDark ? 'dark' : 'light');
        html.classList.toggle('dark', nextIsDark);
        document.getElementById('themeIconSun').classList.toggle('hidden', !nextIsDark);
        document.getElementById('themeIconMoon').classList.toggle('hidden', nextIsDark);
        renderSidebarSection();
        renderSeparateTabStrip();
        if (!document.getElementById('webInspectorDrawer').classList.contains('hidden')) {
          renderWebInspector();
        }
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.setTheme(nextIsDark);
          var tTab = getActiveTab();
          var tUrl = tTab.history[tTab.historyIndex];
          window.aetherBridge.syncAiChatContext(getPageData(tUrl).title, tUrl, nextIsDark);
        }
      }

      function togglePrivateBrowsing() {
        if (!isPrivateMode) {
          saveBrowserSession();
          // Preserve normal browser tabs without sharing their actual WebView2 profiles.
          normalTabsBeforePrivate = { tabs: tabs, activeTabId: activeTabId };
          var privateTabId = 'tab-private-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8);
          tabs = [{
            id: privateTabId, group: 'Private Tabs', history: ['browsphere://start'],
            historyIndex: 0, privateContext: true
          }];
          activeTabId = privateTabId;
          isPrivateMode = true;
        } else {
          var disposablePrivateTabs = tabs.slice();
          isPrivateMode = false;
          if (normalTabsBeforePrivate) {
            tabs = normalTabsBeforePrivate.tabs;
            activeTabId = normalTabsBeforePrivate.activeTabId;
          } else {
            tabs = [{ id: 'tab-1', group: 'Open Tabs', history: ['browsphere://start'], historyIndex: 0 }];
            activeTabId = 'tab-1';
          }
          normalTabsBeforePrivate = null;
          // Close private WebView2 profiles; normal tabs and their cookies are untouched.
          if (window.aetherBridge && window.aetherBridge.isNative) {
            disposablePrivateTabs.forEach(function (privateTab) {
              markLuciaTaskTabClosed(privateTab.id, 'private_session_end');
              window.aetherBridge.onCloseTab(privateTab.id, 'private_session_end');
            });
          }
          saveBrowserSession();
        }
        document.documentElement.setAttribute('data-private', isPrivateMode ? 'true' : 'false');
        document.getElementById('privateModeBadge').classList.toggle('hidden', !isPrivateMode);
        document.getElementById('privateTogglePill').textContent = isPrivateMode ? 'On' : 'Off';
        renderActiveTab(true);
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var tab = getActiveTab();
          window.aetherBridge.onSwitchTab(tab.id, tab.history[tab.historyIndex]);
        }
        showHudToast(isPrivateMode
          ? 'Private tabs use isolated WebView2 incognito sessions'
          : 'Private tabs closed; ordinary browsing restored');
      }

      function toggleFullscreenWindow() {
        var win = document.getElementById('browsphereWindow');
        win.classList.toggle('max-w-[1400px]');
        win.classList.toggle('max-w-full');
        win.classList.toggle('h-[93vh]');
        win.classList.toggle('h-[97vh]');
      }

      // Uses the official Hacker News Firebase API: actual published stories, not mock headlines.
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
        if (newTabNewsLoading || (!force && Date.now() - newTabNewsLastFetch < 12000)) return;
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
            var incomingIds = ids.slice(0, 35);
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
            }
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

      function setupScrollProgress() {
        var canvas = document.getElementById('readingCanvas');
        var bar = document.getElementById('scrollProgressBar');
        canvas.addEventListener('scroll', function () {
          if (document.getElementById('newTabNewsSection') && canvas.scrollTop > 80) startLiveNewsStream();
          var max = canvas.scrollHeight - canvas.clientHeight;
          var pct = max > 0 ? Math.min(100, Math.max(12, Math.round((canvas.scrollTop / max) * 100))) : 100;
          bar.style.width = pct + '%';
        });
      }

      function setupKeyboardShortcuts() {
        document.addEventListener('keydown', function (e) {
          var mod = e.metaKey || e.ctrlKey;
          if (mod && e.key === ',') {
            e.preventDefault();
            openSettingsModal('general');
          } else if (mod && e.altKey && e.key.toLowerCase() === 'i') {
            e.preventDefault();
            toggleWebInspector();
          } else if (mod && e.shiftKey && e.key.toLowerCase() === 'l') {
            e.preventDefault();
            toggleSidebar();
          } else if (mod && e.shiftKey && e.key === '\\') {
            e.preventDefault();
            toggleTabOverview();
          } else if (mod && e.key.toLowerCase() === 'j') {
            e.preventDefault();
            togglePopover('aiChatPopover');
          } else if (mod && e.key.toLowerCase() === 'l') {
            e.preventDefault();
            document.getElementById('omnibarInput').focus();
          } else if (mod && e.key.toLowerCase() === 't') {
            e.preventDefault();
            createNewTab();
          } else if (mod && e.key.toLowerCase() === 'w') {
            e.preventDefault();
            closeTab(activeTabId);
          } else if ((mod && e.key.toLowerCase() === 'r') || e.key === 'F5') {
            e.preventDefault();
            reloadActivePage();
          } else if (mod && e.key === 'Tab') {
            e.preventDefault();
            var tabIndex = tabs.findIndex(function (t) { return t.id === activeTabId; });
            var step = e.shiftKey ? -1 : 1;
            switchToTab(tabs[(tabIndex + step + tabs.length) % tabs.length].id);
          } else if (e.altKey && !mod && e.key === 'ArrowLeft') {
            e.preventDefault();
            goHistory(-1);
          } else if (e.altKey && !mod && e.key === 'ArrowRight') {
            e.preventDefault();
            goHistory(1);
          } else if (mod && e.key.toLowerCase() === 'f') {
            e.preventDefault();
            toggleFindBar(true);
          } else if (mod && e.key.toLowerCase() === 'd') {
            e.preventDefault();
            bookmarkCurrentPage();
          } else if (e.key === 'Escape') {
            closeAllPopovers();
            toggleNewTabHistoryDrawer(false);
            hideQuickPreview(true);
            closeSettingsModal();
            toggleTabOverview(false);
            toggleFindBar(false);
          }
        });
      }

      /* ========================================================================
         11. AUTHENTIC MACOS SAFARI SETTINGS WINDOW (⌘,) & LIVE WIRING
         ======================================================================== */
      function openSettingsModal(pane) {
        closeAllPopovers();
        var modal = document.getElementById('browsphereSettingsModal');
        modal.classList.remove('hidden');
        modal.classList.add('flex');
        switchSettingsPane(pane || 'general');
      }

      function closeSettingsModal() {
        var modal = document.getElementById('browsphereSettingsModal');
        modal.classList.add('hidden');
        modal.classList.remove('flex');
      }

      function switchSettingsPane(pane) {
        activeSettingsPane = pane;
        var prefsWindow = document.querySelector('#browsphereSettingsModal .macos-prefs-window');
        var prefsBody = document.getElementById('settingsPaneBody');
        if (prefsWindow) {
          prefsWindow.classList.toggle('max-w-[820px]', pane === 'ai');
          prefsWindow.classList.toggle('max-w-[640px]', pane !== 'ai');
        }
        if (prefsBody) {
          prefsBody.classList.toggle('mac-ai-host', pane === 'ai');
          prefsBody.classList.toggle('max-h-[72vh]', pane === 'ai');
          prefsBody.classList.toggle('max-h-[65vh]', pane !== 'ai');
        }
        var footerNote = document.getElementById('settingsFooterNote');
        if (footerNote) footerNote.textContent = pane === 'ai' ? 'AI settings are saved when you select Save changes.' : 'Changes are saved automatically.';
        var titles = {
          general: 'General',
          tabs: 'Tabs',
          privacy: 'Privacy',
          ai: 'AI & ML',
          extensions: 'Extensions',
          passwords: 'Passwords',
          advanced: 'Advanced'
        };
        document.getElementById('settingsWindowTitle').textContent = titles[pane] || 'General';

        Object.keys(titles).forEach(function (key) {
          var btn = document.getElementById('settingsPaneBtn-' + key);
          if (!btn) return;
          btn.classList.toggle('active', key === pane);
        });

        renderSettingsPane();
        if (pane === 'privacy') renderPrivacyMemories();
        if (pane === 'ai') {
          hydrateRouterFromNative();
          hydrateAwsFromNative();
          refreshNativeOllamaModels(false);
          if (window.LuciaDiagnosticsDashboard) window.LuciaDiagnosticsDashboard.refresh();
        }
        if (pane === 'passwords') hydrateCredentialsFromNative();
      }

      function updateSetting(key, value) {
        browserSettings[key] = value;
        if (key === 'autoMemoryEnabled') {
          try { localStorage.setItem('browsphere-auto-memory', value ? 'on' : 'off'); } catch (_) {}
        }
        applyBrowserSettings();
        renderSettingsPane();
      }

      function applyBrowserSettings() {
        var viewport = document.getElementById('pageViewport');
        if (viewport) {
          viewport.style.zoom = browserSettings.defaultZoom + '%';
        }

        var omnibar = document.getElementById('omnibarInput');
        if (omnibar) {
          omnibar.placeholder = 'Search with ' + browserSettings.searchEngine + ', enter website, or ask AI...';
          refreshOmnibar(false);
        }

        renderSeparateTabStrip();

        var aiLabel = document.getElementById('sidebarAiEngineLabel');
        if (aiLabel) {
          aiLabel.textContent = browserSettings.aiEngine;
        }

        var devBtn = document.getElementById('devInspectorToolbarBtn');
        if (devBtn) {
          devBtn.classList.toggle('hidden', !browserSettings.showDeveloperMenu);
        }

        document.documentElement.setAttribute('data-reduced-motion', browserSettings.reducedMotion ? 'true' : 'false');
      }

      function renderSeparateTabStrip() {
        var strip = document.getElementById('separateTabStrip');
        if (!strip) return;
        if (browserSettings.tabLayout !== 'Separate') {
          strip.classList.add('hidden');
          strip.classList.remove('flex');
          return;
        }
        strip.classList.remove('hidden');
        strip.classList.add('flex');
        strip.innerHTML = tabs.map(function (t) {
          var p = getPageData(t.history[t.historyIndex]);
          var active = t.id === activeTabId;
          var cls = active
            ? 'bg-white dark:bg-zinc-800 font-semibold shadow-2xs border border-black/10 dark:border-white/10'
            : 'opacity-70 hover:opacity-100 hover:bg-black/5 dark:hover:bg-white/10';
          return (
            '<div data-action="switch-tab" data-tab-id="' + t.id + '" class="flex items-center justify-between gap-2 px-3 py-1 rounded-lg max-w-[190px] cursor-pointer transition ' + cls + '">' +
              '<span class="truncate">' + esc(getTabDisplayTitle(t)) + '</span>' +
              '<button type="button" data-action="close-tab" data-tab-id="' + t.id + '" class="opacity-50 hover:opacity-100 text-[10px]">✕</button>' +
            '</div>'
          );
        }).join('');
      }

      function renderPrivacyMemories() {
        var output = document.getElementById('privacyMemoryList');
        if (!output) return;
        if (!(window.aetherBridge && window.aetherBridge.isNative)) {
          output.textContent = 'Memory storage is available in the Windows app.';
          return;
        }
        output.textContent = 'Loading saved memories...';
        window.aetherBridge.listMemories().then(function (entries) {
          if (output !== document.getElementById('privacyMemoryList')) return;
          if (!entries.length) {
            output.textContent = 'No saved AI memories yet.';
            return;
          }
          output.innerHTML = entries.map(function (m) {
            return '<div class="flex items-start justify-between gap-3 py-2.5 border-b border-black/10 dark:border-white/10">' +
              '<div class="min-w-0"><div class="text-[10px] font-semibold opacity-60">' + esc((m.kind || 'legacy').toUpperCase()) + ' · ' + esc(m.scope) + (m.evidence_count > 1 ? ' · used ' + esc(m.evidence_count) + ' times' : '') + '</div>' +
              '<p class="text-[12px] leading-relaxed break-words mt-1">' + esc(m.content) + '</p></div>' +
              '<button type="button" class="text-rose-600 dark:text-rose-400 shrink-0 text-[11px] hover:underline" data-action="delete-learned-memory" data-memory-id="' + esc(m.id) + '">Delete</button></div>';
          }).join('');
        }).catch(function (error) {
          output.textContent = 'Unable to load memories: ' + String(error);
        });
      }

      window.addEventListener('aether:memories-updated', function () {
        if (activeSettingsPane === 'privacy') renderPrivacyMemories();
      });

      function renderSettingsPane() {
        var body = document.getElementById('settingsPaneBody');
        var pane = activeSettingsPane;

        if (pane === 'general') {
          var searchOpts = ['Google', 'DuckDuckGo', 'Perplexity', 'Kagi'].map(function (eng) {
            return '<option value="' + eng + '" ' + (browserSettings.searchEngine === eng ? 'selected' : '') + '>' + eng + '</option>';
          }).join('');

          var newTabOpts = ['Start Page', 'Homepage', 'Empty Page'].map(function (opt) {
            return '<option value="' + opt + '" ' + (browserSettings.newTabsOpenWith === opt ? 'selected' : '') + '>' + opt + '</option>';
          }).join('');

          var zoomOpts = [85, 90, 100, 110, 115, 125, 150].map(function (z) {
            return '<option value="' + z + '" ' + (browserSettings.defaultZoom === z ? 'selected' : '') + '>' + z + '%</option>';
          }).join('');

          body.innerHTML =
            '<div class="space-y-3">' +
              '<div class="grid grid-cols-[175px_1fr] items-center gap-x-3 gap-y-2.5">' +
                '<label class="text-right font-medium">Search engine:</label>' +
                '<div><select data-setting-key="searchEngine" class="macos-select w-48">' + searchOpts + '</select></div>' +
                '<label class="text-right font-medium">New tabs open with:</label>' +
                '<div><select data-setting-key="newTabsOpenWith" class="macos-select w-48">' + newTabOpts + '</select></div>' +
                '<label class="text-right font-medium">Homepage:</label>' +
                '<div class="space-y-1.5">' +
                  '<input type="text" data-setting-key="homepageUrl" value="' + esc(browserSettings.homepageUrl) + '" class="macos-text-input w-full max-w-[280px] font-mono text-[12px]" />' +
                  '<div><button type="button" data-action="set-current-home" class="macos-push-btn">Set to Current Page</button></div>' +
                '</div>' +
                '<label class="text-right font-medium">Default page zoom:</label>' +
                '<div><select data-setting-key="defaultZoom" data-setting-type="number" class="macos-select w-28">' + zoomOpts + '</select></div>' +
              '</div>' +
              '<div class="border-t border-black/10 dark:border-white/10 my-3"></div>' +
              '<div class="grid grid-cols-[175px_1fr] items-center gap-x-3">' +
                '<span class="text-right font-medium">History &amp; cache:</span>' +
                '<div><button type="button" data-action="clear-site-data" class="macos-push-btn">Clear History and Website Data...</button></div>' +
              '</div>' +
            '</div>';

        } else if (pane === 'tabs') {
          var closeOpts = ['Manually', 'After one day', 'After one week', 'After one month'].map(function (v) {
            return '<option value="' + v + '" ' + (browserSettings.autoCloseTabs === v ? 'selected' : '') + '>' + v + '</option>';
          }).join('');

          var compactBorder = browserSettings.tabLayout === 'Compact' ? 'border-[#007AFF] ring-2 ring-[#007AFF]/25' : 'border-black/15 dark:border-white/15';
          var sepBorder = browserSettings.tabLayout === 'Separate' ? 'border-[#007AFF] ring-2 ring-[#007AFF]/25' : 'border-black/15 dark:border-white/15';

          body.innerHTML =
            '<div class="space-y-3">' +
              '<div class="grid grid-cols-[175px_1fr] items-start gap-x-3 gap-y-3">' +
                '<span class="text-right font-medium pt-1">Tab layout:</span>' +
                '<div class="flex items-start gap-4">' +
                  '<label class="cursor-pointer flex flex-col items-center gap-1.5">' +
                    '<div class="w-32 h-16 rounded-[6px] border ' + compactBorder + ' bg-white dark:bg-zinc-800 p-1.5 flex gap-1">' +
                      '<div class="w-7 h-full rounded-[3px] bg-black/10 dark:bg-white/15"></div>' +
                      '<div class="flex-1 flex flex-col gap-1"><div class="h-2.5 rounded-[2px] bg-black/10 dark:bg-white/15"></div><div class="flex-1 rounded-[2px] bg-black/[0.03] dark:bg-white/[0.04]"></div></div>' +
                    '</div>' +
                    '<span class="flex items-center gap-1.5 text-[12px]"><input type="radio" name="tabLayoutRadio" data-action="set-tab-layout" data-layout="Compact" class="macos-radio" ' + (browserSettings.tabLayout === 'Compact' ? 'checked' : '') + ' /><span>Compact</span></span>' +
                  '</label>' +
                  '<label class="cursor-pointer flex flex-col items-center gap-1.5">' +
                    '<div class="w-32 h-16 rounded-[6px] border ' + sepBorder + ' bg-white dark:bg-zinc-800 p-1.5 flex flex-col gap-1">' +
                      '<div class="h-2.5 rounded-[2px] bg-black/10 dark:bg-white/15"></div>' +
                      '<div class="h-2 rounded-[2px] bg-[#007AFF]/25"></div>' +
                      '<div class="flex-1 rounded-[2px] bg-black/[0.03] dark:bg-white/[0.04]"></div>' +
                    '</div>' +
                    '<span class="flex items-center gap-1.5 text-[12px]"><input type="radio" name="tabLayoutRadio" data-action="set-tab-layout" data-layout="Separate" class="macos-radio" ' + (browserSettings.tabLayout === 'Separate' ? 'checked' : '') + ' /><span>Separate</span></span>' +
                  '</label>' +
                '</div>' +
                '<span class="text-right font-medium">Tab groups:</span>' +
                '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="smartAiGrouping" class="macos-checkbox" ' + (browserSettings.smartAiGrouping ? 'checked' : '') + ' /><span>Automatically group related research tabs</span></label>' +
                '<label class="text-right font-medium">Automatically close tabs:</label>' +
                '<div><select data-setting-key="autoCloseTabs" class="macos-select w-44">' + closeOpts + '</select></div>' +
              '</div>' +
            '</div>';

        } else if (pane === 'privacy') {
          body.innerHTML =
            '<div class="space-y-3">' +
              '<div class="grid grid-cols-[175px_1fr] items-start gap-x-3 gap-y-2.5">' +
                '<span class="text-right font-medium">Website tracking:</span>' +
                '<div class="space-y-2">' +
                  '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="preventCrossSiteTracking" class="macos-checkbox" ' + (browserSettings.preventCrossSiteTracking ? 'checked' : '') + ' /><span>Prevent cross-site tracking</span></label>' +
                  '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="hideIpFromTrackers" class="macos-checkbox" ' + (browserSettings.hideIpFromTrackers ? 'checked' : '') + ' /><span>Hide IP address from trackers</span></label>' +
                '</div>' +
                '<span class="text-right font-medium pt-1">Cookies and website data:</span>' +
                '<div class="space-y-2 pt-1">' +
                  '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="blockAllCookies" class="macos-checkbox" ' + (browserSettings.blockAllCookies ? 'checked' : '') + ' /><span>Block all cookies</span></label>' +
                  '<div><button type="button" data-action="toast" data-msg="Removed cached website data for 4 domains" class="macos-push-btn">Manage Website Data...</button></div>' +
                '</div>' +
                '<span class="text-right font-medium pt-1">Security:</span>' +
                '<label class="flex items-center gap-2 cursor-pointer pt-1"><input type="checkbox" data-setting-key="fraudulentWarning" class="macos-checkbox" ' + (browserSettings.fraudulentWarning ? 'checked' : '') + ' /><span>Warn when visiting a fraudulent website</span></label>' +
              '</div>' +
              '<div class="mt-6 pt-4 border-t border-black/10 dark:border-white/10 space-y-3">' +
                '<label class="flex items-center gap-2 cursor-pointer text-[12px]"><input type="checkbox" data-setting-key="autoMemoryEnabled" class="macos-checkbox" ' + (browserSettings.autoMemoryEnabled ? 'checked' : '') + ' /><span>Automatically learn from completed AI tasks</span></label>' +
                '<div class="flex flex-wrap justify-between items-center gap-2"><div><h3 class="font-semibold text-[13px]">Automatic AI memory</h3><p class="text-[11px] opacity-65 mt-1">Browsphere learns limited, non-sensitive patterns from completed tasks. Private Browsing does not save memories.</p></div><button type="button" class="macos-push-btn text-rose-600" data-action="clear-learned-memories">Clear All Memories</button></div>' +
                '<div id="privacyMemoryList" aria-live="polite" class="max-h-44 overflow-auto rounded-lg border border-black/10 dark:border-white/10 px-3 py-2 text-[12px] opacity-90">Loading saved memories...</div>' +
              '</div>' +
            '</div>';

        } else if (pane === 'ai') {
          body.innerHTML = window.renderMacAiPreferences({
            config: nativeRouterConfig,
            localModels: nativeOllamaModels,
            localModelsError: nativeOllamaModelsError,
            hasKey: nativeHasLiteLlmKey,
            testStatus: nativeModelTestStatus,
            fetchStatus: nativeCloudFetchStatus,
            aws: nativeAwsConfig,
            awsSession: nativeAwsSession,
            awsStatus: nativeAwsStatus,
            awsAccountHint: nativeAwsAccountHint,
            awsServices: nativeAwsServices,
            awsConnecting: nativeAwsConnecting
          });

        } else if (pane === 'extensions') {
          if (!browserSettings.extensions.length) {
            body.innerHTML = '<div class="macos-group-box p-5 text-[12px] space-y-2">' +
              '<h3 class="font-semibold text-[14px]">Extensions are not available yet</h3>' +
              '<p class="opacity-70">Browsphere does not currently install or run Chrome, Edge or Safari extensions. The previously displayed examples were placeholders, not working integrations.</p>' +
              '<p class="opacity-70">Extension hosting, permission handling and security review are pending for a later release.</p>' +
              '</div>';
            return;
          }
          var activeExt = browserSettings.extensions[selectedExtensionIdx] || browserSettings.extensions[0];
          var listHtml = browserSettings.extensions.map(function (ext, idx) {
            var isSelected = idx === selectedExtensionIdx;
            var rowCls = isSelected ? 'bg-[#007AFF] text-white' : 'hover:bg-black/5 dark:hover:bg-white/5';
            return (
              '<div data-action="select-extension" data-ext-select="' + idx + '" class="flex items-center gap-2.5 px-2.5 py-2 cursor-pointer text-[12.5px] ' + rowCls + '">' +
                '<input type="checkbox" data-ext-index="' + idx + '" class="macos-checkbox" ' + (ext.enabled ? 'checked' : '') + ' />' +
                '<span class="font-medium truncate">' + esc(ext.name) + '</span>' +
              '</div>'
            );
          }).join('');

          body.innerHTML =
            '<div class="grid grid-cols-[220px_1fr] gap-3.5 h-[220px]">' +
              '<div class="macos-group-box overflow-y-auto divide-y divide-black/5 dark:divide-white/5">' + listHtml + '</div>' +
              '<div class="macos-group-box p-4 flex flex-col justify-between">' +
                '<div class="space-y-2">' +
                  '<div class="flex items-center justify-between">' +
                    '<span class="font-semibold text-[13.5px]">' + esc(activeExt.name) + '</span>' +
                    '<span class="text-[11px] opacity-60">' + (activeExt.enabled ? 'Enabled' : 'Disabled') + '</span>' +
                  '</div>' +
                  '<p class="text-[12px] opacity-75 leading-relaxed">' + esc(activeExt.desc) + '</p>' +
                  '<div class="pt-2 text-[11.5px] opacity-60">Permissions: Read and alter webpage content on allowed websites.</div>' +
                '</div>' +
                '<div class="pt-3 border-t border-black/10 dark:border-white/10 flex items-center justify-between text-[11.5px]">' +
                  '<span class="opacity-60">Signed WebKit Extension</span>' +
                  '<button type="button" data-action="toast" data-msg="Extension preferences opened" class="macos-push-btn">Preferences...</button>' +
                '</div>' +
              '</div>' +
            '</div>';

        } else if (pane === 'passwords') {
          var isNativeVault = Boolean(window.aetherBridge && window.aetherBridge.isNative);
          var sourceList = isNativeVault ? nativeCredentialsMeta : browserSettings.savedLogins;
          var filtered = sourceList.filter(function (item) {
            return item.site.toLowerCase().indexOf(passwordSearchQuery.toLowerCase()) !== -1 ||
                   item.username.toLowerCase().indexOf(passwordSearchQuery.toLowerCase()) !== -1;
          });

          var rowsHtml = filtered.length ? filtered.map(function (item) {
            var key = item.site + '::' + item.username;
            var isRevealed = !!revealedPasswords[key];
            var shownVal = isNativeVault
              ? (isRevealed ? esc(nativeRevealedCache[key] || '(empty)') : '••••••••••••')
              : (isRevealed ? esc(item.plain) : item.masked);
            return (
              '<div class="grid grid-cols-[135px_1fr_195px] items-center px-3 py-1.5 text-[12px]">' +
                '<span class="font-medium truncate pr-2">' + esc(item.site) + '</span>' +
                '<span class="opacity-75 truncate pr-2">' + esc(item.username) + '</span>' +
                '<div class="flex items-center justify-between gap-1">' +
                  '<span class="font-mono text-[11px] truncate">' + shownVal + '</span>' +
                  '<div class="flex items-center gap-1 shrink-0">' +
                    '<button type="button" data-action="reveal-password" data-site="' + esc(item.site) + '" data-username="' + esc(item.username) + '" class="macos-push-btn !px-1.5 !py-0 text-[11px]">' + (isRevealed ? 'Hide' : 'Show') + '</button>' +
                    '<button type="button" data-action="copy-password" data-site="' + esc(item.site) + '" data-username="' + esc(item.username) + '" data-secret="' + esc(item.plain || '') + '" class="macos-push-btn !px-1.5 !py-0 text-[11px]">Copy</button>' +
                    (isNativeVault
                      ? '<button type="button" data-action="autofill-tab" data-site="' + esc(item.site) + '" data-username="' + esc(item.username) + '" title="Fill into active tab login form" class="macos-push-btn !px-1.5 !py-0 text-[11px] text-[#007AFF] font-medium">Fill</button>' +
                        '<button type="button" data-action="delete-credential" data-site="' + esc(item.site) + '" data-username="' + esc(item.username) + '" class="macos-push-btn !px-1.5 !py-0 text-[11px] text-rose-500">✕</button>'
                      : '') +
                  '</div>' +
                '</div>' +
              '</div>'
            );
          }).join('') : '<div class="px-3 py-5 text-center text-[11.5px] opacity-55">No saved credentials in Windows Credential Manager yet.</div>';

          body.innerHTML =
            '<div class="space-y-3">' +
              '<div class="flex items-center justify-between gap-3">' +
                '<label class="flex items-center gap-2 cursor-pointer text-[12.5px]"><input type="checkbox" data-setting-key="autofillPasswords" class="macos-checkbox" ' + (browserSettings.autofillPasswords ? 'checked' : '') + ' /><span>AutoFill (Windows Credential Manager)</span></label>' +
                '<input id="settingsPasswordSearch" type="text" value="' + esc(passwordSearchQuery) + '" placeholder="Search" class="macos-text-input w-44 text-[12px]" />' +
              '</div>' +
              '<div class="grid grid-cols-[1fr_1fr_1fr_auto] gap-1.5 items-center">' +
                '<input id="credNewSite" type="text" placeholder="Website (e.g. github.com)" class="macos-text-input text-[12px]" />' +
                '<input id="credNewUser" type="text" placeholder="Username / Email" class="macos-text-input text-[12px]" />' +
                '<input id="credNewPass" type="password" placeholder="Password" class="macos-text-input text-[12px]" />' +
                '<button type="button" data-action="add-credential" class="macos-push-btn font-medium">+ Save</button>' +
              '</div>' +
              '<div class="macos-group-box overflow-hidden">' +
                '<div class="grid grid-cols-[135px_1fr_195px] px-3 py-1.5 border-b border-black/10 dark:border-white/10 text-[11px] font-semibold opacity-65 bg-black/[0.02] dark:bg-white/[0.02]">' +
                  '<span>Website</span><span>User Name</span><span>Password</span>' +
                '</div>' +
                '<div class="divide-y divide-black/5 dark:divide-white/5 max-h-[165px] overflow-y-auto">' + rowsHtml + '</div>' +
              '</div>' +
            '</div>';

        } else if (pane === 'advanced') {
          body.innerHTML =
            '<div class="space-y-3">' +
              '<div class="grid grid-cols-[175px_1fr] items-start gap-x-3 gap-y-2.5">' +
                '<span class="text-right font-medium">Smart Search Field:</span>' +
                '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="showFullWebsiteAddress" class="macos-checkbox" ' + (browserSettings.showFullWebsiteAddress ? 'checked' : '') + ' /><span>Show full website address</span></label>' +
                '<span class="text-right font-medium">Accessibility:</span>' +
                '<label class="flex items-center gap-2 cursor-pointer"><input type="checkbox" data-setting-key="reducedMotion" class="macos-checkbox" ' + (browserSettings.reducedMotion ? 'checked' : '') + ' /><span>Reduce interface motion and window animations</span></label>' +
                '<span class="text-right font-medium pt-1">Developer:</span>' +
                '<label class="flex items-center gap-2 cursor-pointer pt-1"><input type="checkbox" data-setting-key="showDeveloperMenu" class="macos-checkbox" ' + (browserSettings.showDeveloperMenu ? 'checked' : '') + ' /><span>Show features for web developers (Web Inspector ⌥⌘I)</span></label>' +
              '</div>' +
            '</div>';
        }
      }

      /* ========================================================================
         12. DEVELOPER WEB INSPECTOR DRAWER (⌥⌘I)
         ======================================================================== */
      function toggleWebInspector(force) {
        var drawer = document.getElementById('webInspectorDrawer');
        var show = typeof force === 'boolean' ? force : drawer.classList.contains('hidden');
        if (show) {
          drawer.classList.remove('hidden');
          drawer.classList.add('flex');
          hydrateTracesFromNative();
          hydrateAutomationsFromNative();
          renderWebInspector();
        } else {
          drawer.classList.add('hidden');
          drawer.classList.remove('flex');
        }
      }

      function switchInspectorTab(tabName) {
        activeInspectorTab = tabName;
        ['elements', 'console', 'network'].forEach(function (k) {
          var btn = document.getElementById('inspTab-' + k);
          if (!btn) return;
          btn.className = k === tabName
            ? 'px-2.5 py-0.5 rounded-md bg-[#007AFF] text-white font-semibold text-[11px]'
            : 'px-2.5 py-0.5 rounded-md opacity-70 hover:opacity-100 font-medium text-[11px]';
        });
        var form = document.getElementById('inspectorConsoleForm');
        if (tabName === 'console') {
          form.classList.remove('hidden');
          form.classList.add('flex');
        } else {
          form.classList.add('hidden');
          form.classList.remove('flex');
        }
        renderWebInspector();
      }

      function renderWebInspector() {
        var body = document.getElementById('webInspectorBody');
        var tab = getActiveTab();
        var url = tab.history[tab.historyIndex];
        var page = getPageData(url);

        if (activeInspectorTab === 'elements') {
          body.innerHTML =
            '<div class="text-sky-600 dark:text-sky-400">&lt;!DOCTYPE html&gt;</div>' +
            '<div class="pl-2 text-purple-600 dark:text-purple-400">&lt;html lang="en" data-theme="' + esc(document.documentElement.getAttribute('data-theme')) + '"&gt;</div>' +
            '<div class="pl-4 opacity-75">&lt;head&gt;&lt;title&gt;' + esc(page.title) + '&lt;/title&gt;&lt;/head&gt;</div>' +
            '<div class="pl-4 text-purple-600 dark:text-purple-400">&lt;body&gt;</div>' +
            '<div class="pl-6 bg-[#007AFF]/12 text-[#007AFF] rounded px-1.5 py-0.5">&lt;main id="readingCanvas" data-url="' + esc(url) + '" data-zoom="' + browserSettings.defaultZoom + '%"&gt; ... &lt;/main&gt;</div>' +
            '<div class="pl-6 opacity-75">&lt;aside id="aiChatPopover" data-engine="' + esc(browserSettings.aiEngine) + '"&gt;&lt;/aside&gt;</div>' +
            '<div class="pl-4 text-purple-600 dark:text-purple-400">&lt;/body&gt;</div>' +
            '<div class="pl-2 text-purple-600 dark:text-purple-400">&lt;/html&gt;</div>';
        } else if (activeInspectorTab === 'console') {
          body.innerHTML = inspectorConsoleLogs.map(function (entry) {
            var cls = entry.level === 'error' ? 'text-rose-500' : entry.level === 'info' ? 'text-[#007AFF]' : '';
            return '<div class="py-0.5 border-b border-black/5 dark:border-white/5 ' + cls + '">' + esc(entry.text) + '</div>';
          }).join('');
        } else if (activeInspectorTab === 'network') {
          var runOpts = '<option value="all">All Runs (' + nativeRuns.length + ')</option>' + nativeRuns.map(function (r) {
            var lbl = '[' + r.status + '] ' + (r.goal.length > 32 ? r.goal.slice(0, 32) + '…' : r.goal) + ' (' + r.step_count + ' steps)';
            return '<option value="' + esc(r.run_id) + '" ' + (traceFilterRunId === r.run_id ? 'selected' : '') + '>' + esc(lbl) + '</option>';
          }).join('');

          var riskOpts = ['all', 'low', 'medium', 'high', 'critical'].map(function (rk) {
            return '<option value="' + rk + '" ' + (traceFilterRisk === rk ? 'selected' : '') + '>Risk: ' + rk + '</option>';
          }).join('');

          var traceRows = nativeTraces.length
            ? nativeTraces.map(function (t) {
                var riskCls = (t.risk === 'critical' || t.risk === 'high') ? 'text-rose-500 font-semibold' : (t.risk === 'medium' ? 'text-amber-500' : 'text-emerald-500');
                return (
                  '<div class="py-1 border-b border-black/5 dark:border-white/5 space-y-0.5">' +
                    '<div class="flex items-center justify-between gap-2">' +
                      '<span class="font-semibold">Step ' + esc(t.step_index) + ' · ' + esc(t.model_route) + '</span>' +
                      '<span class="' + riskCls + '">' + esc(t.risk) + ' · ' + esc(t.latency_ms) + 'ms</span>' +
                    '</div>' +
                    '<div class="opacity-80 truncate">' + esc(t.thought) + '</div>' +
                    '<div class="opacity-60 text-[10.5px] truncate">' + esc(t.outcome) + ' · ' + esc(t.active_origin) + '</div>' +
                  '</div>'
                );
              }).join('')
            : '<div class="py-4 opacity-55 text-center">No agent execution traces recorded in SQLite WAL yet. Run a task with Lucia.</div>';

          var macroRows = nativeMacros.length
            ? nativeMacros.map(function (m) {
                return (
                  '<div class="flex items-center justify-between gap-2 py-1 px-2 rounded bg-black/[0.03] dark:bg-white/[0.05] text-[11px]">' +
                    '<div class="min-w-0">' +
                      '<div class="font-semibold truncate">' + esc(m.goal_display) + '</div>' +
                      '<div class="opacity-60 text-[10px] truncate">' + esc(m.origin_host) + ' · ' + (m.steps ? m.steps.length : 0) + ' steps · ' + esc(m.hit_count) + ' replays</div>' +
                    '</div>' +
                    '<div class="flex items-center gap-1 shrink-0">' +
                      '<button type="button" data-action="replay-macro" data-macro-id="' + esc(m.macro_id) + '" class="px-2 py-0.5 rounded bg-[#007AFF] text-white text-[10px] font-semibold hover:brightness-110">Replay</button>' +
                      '<button type="button" data-action="delete-macro" data-macro-id="' + esc(m.macro_id) + '" class="px-1.5 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10px] hover:text-rose-500">✕</button>' +
                    '</div>' +
                  '</div>'
                );
              }).join('')
            : '<div class="text-[10.5px] opacity-55">No cached workflows yet. Completed agent runs are cached automatically.</div>';

          body.innerHTML =
            '<div class="flex items-center gap-2 pb-2 mb-1.5 border-b border-black/10 dark:border-white/10 font-sans">' +
              '<select id="inspTraceRunSelect" class="macos-select text-[11px] max-w-[260px]">' + runOpts + '</select>' +
              '<select id="inspTraceRiskSelect" class="macos-select text-[11px]">' + riskOpts + '</select>' +
              '<button type="button" data-action="run-load-test" class="macos-push-btn !px-2 !py-0.5 !text-[10.5px]">Load Test</button>' +
              '<button type="button" data-action="check-updates" class="macos-push-btn !px-2 !py-0.5 !text-[10.5px]">Check Update</button>' +
              '<span class="ml-auto text-[10.5px] opacity-60">Macros: ' + nativeMacros.length + ' · ITP Blocked: ' + nativeItpTotal + '</span>' +
            '</div>' +
            '<div class="mb-2 pb-2 border-b border-black/10 dark:border-white/10 space-y-1 font-sans">' +
              '<div class="text-[10.5px] font-semibold uppercase tracking-wider opacity-55">Cached Workflows (0-LLM Deterministic Replay)</div>' +
              macroRows +
            '</div>' +
            '<div class="space-y-0.5">' + traceRows + '</div>';

          var runSel = document.getElementById('inspTraceRunSelect');
          var riskSel = document.getElementById('inspTraceRiskSelect');
          if (runSel) {
            runSel.addEventListener('change', function () {
              traceFilterRunId = runSel.value;
              hydrateTracesFromNative();
            });
          }
          if (riskSel) {
            riskSel.addEventListener('change', function () {
              traceFilterRisk = riskSel.value;
              hydrateTracesFromNative();
            });
          }
        }
      }

      function handleInspectorConsoleSubmit(e) {
        e.preventDefault();
        var input = document.getElementById('inspectorConsoleInput');
        var code = input.value.trim();
        if (!code) return;
        input.value = '';
        inspectorConsoleLogs.push({ level: 'log', text: '> ' + code });
        if (code === 'document.title') {
          var page = getPageData(getActiveTab().history[getActiveTab().historyIndex]);
          inspectorConsoleLogs.push({ level: 'info', text: '"' + page.title + '"' });
        } else if (code === 'navigator.userAgent') {
          inspectorConsoleLogs.push({ level: 'info', text: '"Mozilla/5.0 (Macintosh; Apple Silicon Mac OS X 15_4) AppleWebKit/605.1.15 Browsphere/18.4 AI-Native"' });
        } else {
          inspectorConsoleLogs.push({ level: 'info', text: 'Evaluated in sandbox: ' + code });
        }
        renderWebInspector();
      }

      var toastTimer = null;
      function showHudToast(msg) {
        var toast = document.getElementById('hudToast');
        toast.textContent = msg;
        toast.classList.remove('opacity-0');
        clearTimeout(toastTimer);
        toastTimer = setTimeout(function () {
          toast.classList.add('opacity-0');
        }, 2000);
      }

      /* ========================================================================
         13. DRAGGABLE & 8-DIRECTION RESIZABLE AI QUICK CHAT WINDOW
         ======================================================================== */
      function setupDraggableResizableChat() {
        var popover = document.getElementById('aiChatPopover');
        var header = document.getElementById('aiChatHeader');
        if (!popover || !header || !popover.parentElement) return;

        var MIN_W = 310;
        var MIN_H = 320;
        var MAX_W = 780;
        var EDGE_MARGIN = 10;

        var activeMode = null;
        var resizeDir = '';
        var startPointerX = 0;
        var startPointerY = 0;
        var startLeft = 0;
        var startTop = 0;
        var startWidth = 0;
        var startHeight = 0;

        function getContainerBounds() {
          var rect = popover.parentElement.getBoundingClientRect();
          return {
            width: rect.width,
            height: rect.height,
            maxAllowedH: Math.max(MIN_H, rect.height - 20)
          };
        }

        function ensureExplicitLeftTop() {
          var containerRect = popover.parentElement.getBoundingClientRect();
          var popRect = popover.getBoundingClientRect();
          var currentLeft = popRect.left - containerRect.left;
          var currentTop = popRect.top - containerRect.top;

          popover.style.right = 'auto';
          popover.style.left = Math.round(currentLeft) + 'px';
          popover.style.top = Math.round(currentTop) + 'px';
          popover.style.width = Math.round(popRect.width) + 'px';
          popover.style.height = Math.round(popRect.height) + 'px';

          return {
            left: currentLeft,
            top: currentTop,
            width: popRect.width,
            height: popRect.height
          };
        }

        header.addEventListener('pointerdown', function (e) {
          if (e.button !== 0 && e.pointerType === 'mouse') return;
          if (e.target.closest('button')) return;

          var rect = ensureExplicitLeftTop();
          activeMode = 'drag';
          startPointerX = e.clientX;
          startPointerY = e.clientY;
          startLeft = rect.left;
          startTop = rect.top;
          startWidth = rect.width;
          startHeight = rect.height;

          popover.classList.add('is-interacting', 'is-dragging');
          if (header.setPointerCapture) header.setPointerCapture(e.pointerId);
        });

        popover.querySelectorAll('.chat-resize-handle').forEach(function (handle) {
          handle.addEventListener('pointerdown', function (e) {
            if (e.button !== 0 && e.pointerType === 'mouse') return;
            e.stopPropagation();

            var rect = ensureExplicitLeftTop();
            activeMode = 'resize';
            resizeDir = handle.dataset.resizeDir || 'se';
            startPointerX = e.clientX;
            startPointerY = e.clientY;
            startLeft = rect.left;
            startTop = rect.top;
            startWidth = rect.width;
            startHeight = rect.height;

            popover.classList.add('is-interacting');
            if (handle.setPointerCapture) handle.setPointerCapture(e.pointerId);
          });
        });

        window.addEventListener('pointermove', function (e) {
          if (!activeMode) return;
          var dx = e.clientX - startPointerX;
          var dy = e.clientY - startPointerY;
          var bounds = getContainerBounds();

          if (activeMode === 'drag') {
            var maxLeft = Math.max(EDGE_MARGIN, bounds.width - startWidth - EDGE_MARGIN);
            var maxTop = Math.max(EDGE_MARGIN, bounds.height - startHeight - EDGE_MARGIN);
            var nextLeft = Math.min(maxLeft, Math.max(EDGE_MARGIN, startLeft + dx));
            var nextTop = Math.min(maxTop, Math.max(EDGE_MARGIN, startTop + dy));

            popover.style.left = Math.round(nextLeft) + 'px';
            popover.style.top = Math.round(nextTop) + 'px';
            return;
          }

          if (activeMode === 'resize') {
            var effectiveMaxW = Math.min(MAX_W, bounds.width - EDGE_MARGIN * 2);
            var effectiveMaxH = bounds.maxAllowedH;

            var nextL = startLeft;
            var nextT = startTop;
            var nextW = startWidth;
            var nextH = startHeight;

            if (resizeDir.indexOf('e') !== -1) {
              var maxWFromLeft = Math.min(effectiveMaxW, bounds.width - startLeft - EDGE_MARGIN);
              nextW = Math.min(maxWFromLeft, Math.max(MIN_W, startWidth + dx));
            }
            if (resizeDir.indexOf('w') !== -1) {
              var rightEdge = startLeft + startWidth;
              var proposedWidth = startWidth - dx;
              var maxWToLeftEdge = Math.min(effectiveMaxW, rightEdge - EDGE_MARGIN);
              nextW = Math.min(maxWToLeftEdge, Math.max(MIN_W, proposedWidth));
              nextL = rightEdge - nextW;
            }
            if (resizeDir.indexOf('s') !== -1) {
              var maxHFromTop = Math.min(effectiveMaxH, bounds.height - startTop - EDGE_MARGIN);
              nextH = Math.min(maxHFromTop, Math.max(MIN_H, startHeight + dy));
            }
            if (resizeDir.indexOf('n') !== -1) {
              var bottomEdge = startTop + startHeight;
              var proposedHeight = startHeight - dy;
              var maxHToTopEdge = Math.min(effectiveMaxH, bottomEdge - EDGE_MARGIN);
              nextH = Math.min(maxHToTopEdge, Math.max(MIN_H, proposedHeight));
              nextT = bottomEdge - nextH;
            }

            popover.style.left = Math.round(nextL) + 'px';
            popover.style.top = Math.round(nextT) + 'px';
            popover.style.width = Math.round(nextW) + 'px';
            popover.style.height = Math.round(nextH) + 'px';
          }
        });

        function endPointerInteraction() {
          if (!activeMode) return;
          activeMode = null;
          resizeDir = '';
          popover.classList.remove('is-interacting', 'is-dragging');
        }

        window.addEventListener('pointerup', endPointerInteraction);
        window.addEventListener('pointercancel', endPointerInteraction);

        window.addEventListener('resize', function () {
          if (popover.style.right === 'auto') {
            var bounds = getContainerBounds();
            var curLeft = parseFloat(popover.style.left) || EDGE_MARGIN;
            var curTop = parseFloat(popover.style.top) || EDGE_MARGIN;
            var curW = Math.min(popover.offsetWidth || 390, bounds.width - EDGE_MARGIN * 2);
            var curH = Math.min(popover.offsetHeight || 500, bounds.maxAllowedH);

            popover.style.width = Math.max(MIN_W, Math.round(curW)) + 'px';
            popover.style.height = Math.max(MIN_H, Math.round(curH)) + 'px';
            popover.style.left = Math.round(Math.min(Math.max(EDGE_MARGIN, curLeft), Math.max(EDGE_MARGIN, bounds.width - curW - EDGE_MARGIN))) + 'px';
            popover.style.top = Math.round(Math.min(Math.max(EDGE_MARGIN, curTop), Math.max(EDGE_MARGIN, bounds.height - curH - EDGE_MARGIN))) + 'px';
          }
        });
      }

      function resetChatWindowLayout() {
        var popover = document.getElementById('aiChatPopover');
        if (!popover || !popover.parentElement) return;

        var containerRect = popover.parentElement.getBoundingClientRect();
        var targetW = 390;
        var targetH = Math.min(500, containerRect.height - 20);
        var targetLeft = Math.max(10, containerRect.width - targetW - 14);
        var targetTop = 10;

        if (popover.style.right !== 'auto') {
          var popRect = popover.getBoundingClientRect();
          popover.style.right = 'auto';
          popover.style.left = Math.round(popRect.left - containerRect.left) + 'px';
          popover.style.top = Math.round(popRect.top - containerRect.top) + 'px';
          popover.style.width = Math.round(popRect.width) + 'px';
          popover.style.height = Math.round(popRect.height) + 'px';
          void popover.offsetWidth;
        }

        popover.classList.add('is-docking');
        popover.style.left = Math.round(targetLeft) + 'px';
        popover.style.top = targetTop + 'px';
        popover.style.width = targetW + 'px';
        popover.style.height = Math.round(targetH) + 'px';

        setTimeout(function () {
          popover.classList.remove('is-docking');
          popover.style.left = '';
          popover.style.top = '10px';
          popover.style.right = '14px';
          popover.style.width = '';
          popover.style.height = '';
        }, 230);

        showHudToast('Lucia chat docked to top-right');
      }

      /* ========================================================================
         14. BOOTSTRAP & BELT-AND-SUSPENDERS WINDOW API
         ======================================================================== */
      window.browsphereApp = {
        navigate: navigateToUrl,
        newTab: createNewTab,
        createTaskTab: createLuciaTaskTab,
        switchTab: switchToTab,
        closeTab: closeTab,
        activeTab: function () {
          var tab = getActiveTab();
          return {
            id: tab.id,
            url: tab.history[tab.historyIndex],
            private: isPrivateMode || Boolean(tab.privateContext)
          };
        },
        toggleChat: function () { togglePopover('aiChatPopover'); },
        openSettings: openSettingsModal,
        toggleTheme: toggleThemeMode
      };


      /* ========================================================================
         13B. PRODUCTION RUST / SQLITE WAL / WINDOWS CREDENTIAL MANAGER HYDRATION
         ======================================================================== */
      function hydrateHistoryFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        window.aetherBridge.getHistory(60).then(function (rows) {
          if (!Array.isArray(rows)) return;
          browsingHistory = rows.map(function (r) {
            var tStr = '';
            try { tStr = new Date(r.visited_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }); } catch (_) {}
            return { title: r.title || r.url, url: r.url.replace(/^https?:\/\//, ''), time: tStr };
          });
          if (sidebarSection === 'history') renderSidebarSection();
          if (getActiveTab().history[getActiveTab().historyIndex] === 'browsphere://start') renderActiveTab();
        }).catch(function () {});
      }

      function hydrateDownloadsFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        window.aetherBridge.getDownloads(30).then(function (rows) {
          if (!Array.isArray(rows)) return;
          downloads = rows;
          renderDownloadsList();
        }).catch(function () {});
      }

      function renderPrivacyReportNative() {
        var badge = document.getElementById('itpTotalBadge');
        var headline = document.getElementById('itpTotalHeadline');
        var list = document.getElementById('itpBreakdownList');
        if (badge) badge.textContent = String(nativeItpTotal);
        if (headline) headline.textContent = nativeItpTotal + ' Trackers Prevented from Profiling You';
        if (list) {
          if (!nativeItpBreakdown.length) {
            list.innerHTML = '<div class="opacity-55">No trackers intercepted yet.</div>';
          } else {
            list.innerHTML = nativeItpBreakdown.map(function (item) {
              return '<div class="flex justify-between"><span>' + esc(item.blocked_host) + '</span><span class="opacity-55 font-mono">Blocked (' + esc(item.count) + ')</span></div>';
            }).join('');
          }
        }
      }

      function hydrateItpFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        Promise.all([
          window.aetherBridge.getItpStats(),
          window.aetherBridge.getItpBreakdown(8)
        ]).then(function (res) {
          if (res[0] && typeof res[0].total_blocked === 'number') {
            nativeItpTotal = res[0].total_blocked;
          }
          if (Array.isArray(res[1])) {
            nativeItpBreakdown = res[1];
          }
          renderPrivacyReportNative();
        }).catch(function () {});
      }

      function updateSidebarAiEngine() {
        var aiLabel = document.getElementById('sidebarAiEngineLabel');
        var aiType = document.getElementById('sidebarAiEngineType');
        var local = Boolean(nativeRouterConfig.prefer_local_for_simple);
        if (aiLabel) aiLabel.textContent = local
          ? ('Ollama: ' + nativeRouterConfig.ollama_model)
          : (nativeAwsConfig.bedrock_enabled
              ? ('Bedrock: ' + nativeAwsConfig.bedrock_model)
              : ('Cloud: ' + nativeRouterConfig.litellm_model));
        if (aiType) aiType.textContent = local ? 'On-Device' : 'Cloud API';
      }

      async function refreshNativeOllamaModels(announce) {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        try {
          var models = await window.aetherBridge.listLocalModels();
          nativeOllamaModels = Array.isArray(models) ? models : [];
          nativeOllamaModelsError = nativeOllamaModels.length
            ? '' : 'Ollama is reachable, but no models are installed.';
          if (activeSettingsPane === 'ai') rerenderAiPreservingDraft();
          if (announce) showModelTestStatus(nativeOllamaModels.length
            ? 'Found ' + nativeOllamaModels.length + ' installed Ollama model(s).'
            : nativeOllamaModelsError);
        } catch (err) {
          nativeOllamaModels = [];
          nativeOllamaModelsError = 'Ollama is offline. Open Ollama, then select Refresh.';
          if (activeSettingsPane === 'ai') rerenderAiPreservingDraft();
          if (announce) showModelTestStatus(nativeOllamaModelsError);
        }
      }

      async function connectNativeAwsAccount() {
        if (nativeAwsConnecting) return;
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          awsStatus('Connect AWS requires the Windows application');
          return;
        }
        var profile = (document.getElementById('cfgAwsProfile')?.value || nativeAwsConfig.profile || 'default').trim();
        if (!profile) { awsStatus('Select an existing AWS CLI profile'); return; }
        nativeAwsConnecting = true;
        nativeAwsStatus = 'Checking AWS sign-in. Your browser will open if authentication is needed…';
        rerenderAiPreservingDraft();
        try {
          var result = await window.aetherBridge.connectAwsAccount(profile);
          nativeAwsConfig = Object.assign({}, nativeAwsConfig, result.config);
          nativeAwsAccountHint = result.account_hint || '';
          nativeAwsServices = Array.isArray(result.services) ? result.services : [];
          nativeAwsSession = null;
          var ready = nativeAwsServices.filter(function (s) { return s.status === 'ready'; }).length;
          nativeAwsStatus = (nativeAwsAccountHint || 'AWS connected') + ' · ' +
            (result.region || nativeAwsConfig.region) + ' · ' + ready + ' service resources discovered';
          updateSidebarAiEngine();
          showHudToast('AWS connected. Discovered settings saved automatically.');
        } catch (err) {
          nativeAwsStatus = 'AWS connection failed: ' + String(err);
          showHudToast(nativeAwsStatus.slice(0, 220));
        } finally {
          nativeAwsConnecting = false;
          renderSettingsPane();
        }
      }

      function awsFormValues() {
        var get = function (id, fallback) {
          var el = document.getElementById(id);
          return el ? el.value.trim() : fallback;
        };
        return {
          profile: get('cfgAwsProfile', nativeAwsConfig.profile) || 'default',
          region: get('cfgAwsRegion', nativeAwsConfig.region) || 'us-east-1',
          bedrock_model: get('cfgAwsModel', nativeAwsConfig.bedrock_model) || 'amazon.nova-lite-v1:0',
          bedrock_enabled: Boolean(document.getElementById('cfgAwsBedrockEnabled')?.checked),
          s3_bucket: get('cfgAwsBucket', nativeAwsConfig.s3_bucket),
          s3_key: get('cfgAwsKey', nativeAwsConfig.s3_key) || 'browsphere/settings-backup.json',
          dynamodb_table: get('cfgAwsTable', nativeAwsConfig.dynamodb_table),
          sync_namespace: get('cfgAwsNamespace', nativeAwsConfig.sync_namespace) || 'default',
          cognito_client_id: get('cfgAwsCognitoClient', nativeAwsConfig.cognito_client_id),
          lambda_function: get('cfgAwsLambda', nativeAwsConfig.lambda_function)
        };
      }

      function awsStatus(message) {
        nativeAwsStatus = message;
        var el = document.getElementById('cfgAwsStatus');
        if (el) el.textContent = message;
        showHudToast(message.slice(0, 180));
      }

      function hydrateAwsFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        Promise.all([window.aetherBridge.loadAwsConfig(), window.aetherBridge.cognitoSession()])
          .then(function (res) {
            if (res[0]) nativeAwsConfig = Object.assign({}, nativeAwsConfig, res[0]);
            nativeAwsSession = res[1] || null;
            updateSidebarAiEngine();
            if (activeSettingsPane === 'ai' && !document.getElementById('browsphereSettingsModal').classList.contains('hidden')) {
              rerenderAiPreservingDraft();
            }
          }).catch(function () {});
      }

      async function saveNativeAwsSettings(quiet) {
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          awsStatus('AWS configuration requires the Windows app');
          return false;
        }
        var cfg = awsFormValues();
        try {
          await window.aetherBridge.saveAwsConfig(cfg);
          nativeAwsConfig = cfg;
          updateSidebarAiEngine();
          if (!quiet) awsStatus('AWS settings saved locally. Cloud actions remain manual.');
          return true;
        } catch (err) {
          awsStatus('AWS settings error: ' + String(err));
          return false;
        }
      }

      async function runNativeAwsAction(action) {
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          awsStatus('AWS actions require the Windows app');
          return;
        }
        var username = document.getElementById('cfgAwsCognitoUser')?.value || '';
        var passwordEl = document.getElementById('cfgAwsCognitoPass');
        var password = passwordEl?.value || '';
        var payload = document.getElementById('cfgAwsLambdaPayload')?.value || '{}';
        var isSignOut = action === 'aws-cognito-logout';
        if (!isSignOut && !(await saveNativeAwsSettings(true))) return;
        if (action === 'aws-s3-backup' && !window.confirm('Upload AI model preferences and memory metadata (not memory contents, history or secrets) to your S3 bucket?')) return;
        if (action === 'aws-s3-restore' && !window.confirm('Replace your local AI model preferences with the S3 backup?')) return;
        if (action === 'aws-dynamo-push' && !window.confirm('Send model preferences and memory metadata to DynamoDB?')) return;
        if (action === 'aws-dynamo-pull' && !window.confirm('Replace your local model preferences with the DynamoDB snapshot?')) return;
        if (action === 'aws-lambda-invoke' && !window.confirm('Invoke AWS Lambda ' + nativeAwsConfig.lambda_function + ' with the JSON payload? This may incur AWS charges.')) return;
        if (passwordEl) passwordEl.value = '';
        awsStatus('Contacting AWS…');
        try {
          var result = '';
          switch (action) {
            case 'aws-test': result = await window.aetherBridge.testAwsConnection(); break;
            case 'aws-s3-backup': result = await window.aetherBridge.backupAwsS3(); break;
            case 'aws-s3-restore': result = await window.aetherBridge.restoreAwsS3(); hydrateRouterFromNative(); break;
            case 'aws-dynamo-push': result = await window.aetherBridge.pushAwsDynamoDb(); break;
            case 'aws-dynamo-pull': result = await window.aetherBridge.pullAwsDynamoDb(); hydrateRouterFromNative(); break;
            case 'aws-cognito-login':
              if (!username.trim() || !password) throw new Error('Enter Cognito username and password');
              nativeAwsSession = await window.aetherBridge.cognitoSignIn(username.trim(), password);
              result = 'Signed in to Cognito as ' + nativeAwsSession.username;
              break;
            case 'aws-cognito-logout':
              await window.aetherBridge.cognitoSignOut();
              nativeAwsSession = null;
              result = 'Signed out locally. Session token discarded.';
              break;
            case 'aws-lambda-invoke': result = await window.aetherBridge.invokeAwsLambda(payload); break;
          }
          awsStatus(result || 'AWS action completed');
        } catch (err) {
          awsStatus('AWS error: ' + String(err));
        } finally {
          // Never persist the password in UI state, DOM, or browser storage.
          password = '';
          if (action === 'aws-cognito-login' || action === 'aws-cognito-logout') {
            rerenderAiPreservingDraft();
            var pass = document.getElementById('cfgAwsCognitoPass');
            if (pass) pass.value = '';
          }
        }
      }

      function hydrateRouterFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        Promise.all([
          window.aetherBridge.loadRouterConfig(),
          window.aetherBridge.hasLiteLlmKey()
        ]).then(function (res) {
          if (res[0]) {
            nativeRouterConfig = Object.assign({}, nativeRouterConfig, res[0]);
            updateSidebarAiEngine();
          }
          nativeHasLiteLlmKey = Boolean(res[1]);
          if (activeSettingsPane === 'ai' && !document.getElementById('browsphereSettingsModal').classList.contains('hidden')) {
            renderSettingsPane();
          }
        }).catch(function () {});
      }

      function hydrateCredentialsFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        window.aetherBridge.listSavedCredentials().then(function (rows) {
          if (Array.isArray(rows)) nativeCredentialsMeta = rows;
          if (activeSettingsPane === 'passwords' && !document.getElementById('browsphereSettingsModal').classList.contains('hidden')) {
            renderSettingsPane();
          }
        }).catch(function () {});
      }

      function hydrateMacrosFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        window.aetherBridge.listWorkflowMacros(25).then(function (rows) {
          if (Array.isArray(rows)) nativeMacros = rows;
          if (activeInspectorTab === 'network' && !document.getElementById('webInspectorDrawer').classList.contains('hidden')) {
            renderWebInspector();
          }
        }).catch(function () {});
      }

      function hydrateTracesFromNative() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) return;
        Promise.all([
          window.aetherBridge.getRecentRuns(20),
          window.aetherBridge.getTracesFiltered(traceFilterRunId, traceFilterRisk, 60),
          window.aetherBridge.listWorkflowMacros(25)
        ]).then(function (res) {
          if (Array.isArray(res[0])) nativeRuns = res[0];
          if (Array.isArray(res[1])) nativeTraces = res[1];
          if (Array.isArray(res[2])) nativeMacros = res[2];
          if (activeInspectorTab === 'network' && !document.getElementById('webInspectorDrawer').classList.contains('hidden')) {
            renderWebInspector();
          }
        }).catch(function () {});
      }

      function rerenderAiPreservingDraft() {
        // Rebuilding the rule chips should never discard unsaved provider settings or a new key.
        var ids = ['cfgOllamaUrl', 'cfgOllamaModel', 'cfgWriterModel', 'cfgVisionModel', 'cfgSupervisorModel', 'cfgPlannerModel', 'cfgLiteLlmUrl',
          'cfgLiteLlmModel', 'cfgProviderPreset', 'cfgLiteLlmKey',
          'cfgAwsProfile', 'cfgAwsRegion', 'cfgAwsModel', 'cfgAwsBucket',
          'cfgAwsKey', 'cfgAwsTable', 'cfgAwsNamespace', 'cfgAwsCognitoClient',
          'cfgAwsCognitoUser', 'cfgAwsCognitoPass', 'cfgAwsLambda', 'cfgAwsLambdaPayload'];
        var values = {};
        ids.forEach(function (id) {
          var element = document.getElementById(id);
          if (element) values[id] = element.value;
        });
        var preferred = document.getElementById('cfgPreferLocal');
        var isLocal = preferred ? preferred.checked : nativeRouterConfig.prefer_local_for_simple;
        var cloudOnlyEl = document.getElementById('cfgLuciaCloudOnly');
        var isCloudOnly = cloudOnlyEl ? cloudOnlyEl.checked : nativeRouterConfig.lucia_cloud_only;
        var bedrockEl = document.getElementById('cfgAwsBedrockEnabled');
        var bedrockValue = bedrockEl ? bedrockEl.checked : nativeAwsConfig.bedrock_enabled;
        renderSettingsPane();
        ids.forEach(function (id) {
          var element = document.getElementById(id);
          if (element && Object.prototype.hasOwnProperty.call(values, id)) element.value = values[id];
        });
        var restoredBedrock = document.getElementById('cfgAwsBedrockEnabled');
        if (restoredBedrock) restoredBedrock.checked = bedrockValue;
        var restoredCloudOnly = document.getElementById('cfgLuciaCloudOnly');
        if (restoredCloudOnly) restoredCloudOnly.checked = Boolean(isCloudOnly);
        setNativeAiPreferred(isLocal ? 'ollama' : 'litellm');
      }

      function addNativeDomainRule() {
        var domEl = document.getElementById('cfgRuleDomain');
        var backEl = document.getElementById('cfgRuleBackend');
        if (!domEl || !backEl) return;
        var clean = domEl.value.trim().toLowerCase().replace(/^https?:\/\//, '').split('/')[0];
        if (!clean) return;
        if (!nativeRouterConfig.domain_overrides) nativeRouterConfig.domain_overrides = {};
        nativeRouterConfig.domain_overrides[clean] = backEl.value;
        domEl.value = '';
        rerenderAiPreservingDraft();
      }

      function setNativeAiPreferred(engine) {
        var local = engine === 'ollama' || engine === 'local';
        var preferred = document.getElementById('cfgPreferLocal');
        var toggle = document.getElementById('cfgPreferLocalToggle');
        if (preferred) preferred.checked = local;
        if (toggle) toggle.checked = local;
        document.querySelectorAll('#macAiSettings [data-action="set-ai-preferred"]').forEach(function (button) {
          button.setAttribute('aria-pressed', String((button.dataset.engine === 'ollama') === local));
        });
      }

      async function changeNativeAiPreference(engine) {
        var wasLocal = Boolean(nativeRouterConfig.prefer_local_for_simple);
        var nextLocal = engine === 'ollama' || engine === 'local';
        setNativeAiPreferred(engine);
        showModelTestStatus(nextLocal
          ? 'Switching to local Ollama…'
          : 'Switching to cloud AI…');
        var saved = await saveNativeRouterSettings(true);
        if (saved) {
          showModelTestStatus(nextLocal
            ? 'Local Ollama is selected. Use Test local connection to verify the model.'
            : 'Cloud AI is selected. Use Test cloud connection to verify the provider.');
        } else {
          setNativeAiPreferred(wasLocal ? 'ollama' : 'litellm');
          showModelTestStatus('Could not switch the engine. Check the settings and try again.');
        }
      }

      var localLuciaProfileBusy = false;
      async function useInstalledLocalModelForLucia(requestedModel) {
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          showModelTestStatus('Local Lucia requires the Windows app.');
          return;
        }
        var button = document.querySelector('[data-action="use-local-lucia-fallback"]');
        if (localLuciaProfileBusy) return;
        localLuciaProfileBusy = true;
        document.querySelectorAll('[data-action="use-local-lucia-profile"], [data-action="use-local-lucia-fallback"]').forEach(function (el) { el.disabled = true; });
        var original = null;
        var switched = false;
        try {
          showModelTestStatus('Checking installed Ollama models...');
          var installed = await window.aetherBridge.listLocalModels();
          if (!Array.isArray(installed) || !installed.length) {
            throw new Error('No installed Ollama models. Start Ollama and refresh its models first.');
          }
          // Prefer the model verified with the native Lucia browser flow.
          var model = installed.includes('qwen2.5:3b-instruct') ? 'qwen2.5:3b-instruct' :
            (installed.includes(nativeRouterConfig.ollama_model) ? nativeRouterConfig.ollama_model :
              installed.find(function (name) { return /qwen|llama|mistral|deepseek|gemma/i.test(name); }));
          if (requestedModel === 'role-team') {
            var requiredRoles = ['qwen2.5:3b-instruct','qwen3.5:0.8b','lfm3:1.7b','lfm3-vl:2b'];
            var missingRoles = requiredRoles.filter(function(id){return !installed.includes(id);});
            if (missingRoles.length) throw new Error('Install missing local role models: ' + missingRoles.join(', '));
            model = 'qwen3.5:0.8b';
          } else if (requestedModel) {
            if (!['lfm3:1.7b', 'qwen3.5:0.8b'].includes(requestedModel)) throw new Error('Unknown local profile');
            if (!installed.includes(requestedModel)) {
              if (requestedModel === 'lfm3:1.7b' && installed.includes('lfm3:latest')) model = 'lfm3:latest';
              else throw new Error('Install the requested model with Ollama first: ' + requestedModel);
            } else model = requestedModel;
          }
          if (!model) model = installed.find(function (name) { return /^lfm3:(1\.7b|latest)$/.test(name); });
          if (!model) throw new Error('No supported text model detected. Install LFM3, Qwen, Llama, Mistral, DeepSeek, or Gemma.');
          original = Object.assign({}, nativeRouterConfig, { litellm_api_key: null });
          var localConfig = Object.assign({}, original, {
            ollama_url: 'http://127.0.0.1:11434',
            ollama_model: model,
            planner_model: requestedModel === 'role-team' ? 'qwen2.5:3b-instruct' : '',
            supervisor_model: requestedModel === 'role-team' ? 'lfm3:1.7b' : original.supervisor_model,
            vision_model: (requestedModel === 'role-team' || requestedModel === 'lfm3:1.7b') && installed.includes('lfm3-vl:2b') ? 'lfm3-vl:2b' : original.vision_model || '',
            prefer_local_for_simple: true,
            lucia_cloud_only: false,
            litellm_api_key: null
          });
          await window.aetherBridge.saveRouterConfig(localConfig);
          switched = true;
          showModelTestStatus('Testing Lucia browser actions with ' + model + '...');
          await window.aetherBridge.testModelConnection('local-agent');
          nativeRouterConfig = localConfig;
          nativeOllamaModels = installed;
          updateSidebarAiEngine();
          renderSettingsPane();
          showModelTestStatus('Connected: Lucia uses ' + model + ' locally. Cloud keys were kept.');
          showHudToast('Lucia local model verified and enabled');
        } catch (err) {
          if (switched && original) {
            try { await window.aetherBridge.saveRouterConfig(original); }
            catch (_) { showHudToast('Could not restore previous AI settings; review AI & ML.'); }
          }
          showModelTestStatus('Connection failed: ' + String(err));
        } finally {
          localLuciaProfileBusy = false;
          document.querySelectorAll('[data-action="use-local-lucia-profile"], [data-action="use-local-lucia-fallback"]').forEach(function (el) { el.disabled = false; });
        }
      }

      async function saveNativeRouterSettings(quiet) {
        var cfg = {
          ollama_url: (document.getElementById('cfgOllamaUrl')?.value || '').trim() || 'http://127.0.0.1:11434',
          ollama_model: (document.getElementById('cfgOllamaModel')?.value || '').trim() || 'qwen3.5:4b',
          planner_model: (document.getElementById('cfgPlannerModel')?.value ?? nativeRouterConfig.planner_model ?? '').trim(),
          writer_model: (document.getElementById('cfgWriterModel')?.value ?? 'qwen2.5:3b-instruct').trim(),
          vision_model: (document.getElementById('cfgVisionModel')?.value || '').trim(),
          supervisor_model: (document.getElementById('cfgSupervisorModel')?.value ?? nativeRouterConfig.supervisor_model ?? 'lfm3:1.7b').trim(),
          litellm_url: (document.getElementById('cfgLiteLlmUrl')?.value || '').trim() || 'http://127.0.0.1:4000/v1/chat/completions',
          litellm_model: (document.getElementById('cfgLiteLlmModel')?.value || '').trim() ||
            (Object.values(window.browsphereCloudPresets || {}).some(function (preset) {
              return preset[0] === (document.getElementById('cfgLiteLlmUrl')?.value || '').trim()
                && preset[1] === '9router/select-model';
            }) ? '9router/select-model' : 'gpt-4o-mini'),
          litellm_api_key: (document.getElementById('cfgLiteLlmKey')?.value || '').trim() || null,
          domain_overrides: Object.assign({}, nativeRouterConfig.domain_overrides || {}),
          prefer_local_for_simple: Boolean(document.getElementById('cfgPreferLocal')?.checked),
          lucia_cloud_only: Boolean(document.getElementById('cfgLuciaCloudOnly')?.checked),
          cloud_model_catalogs: Object.assign({}, nativeRouterConfig.cloud_model_catalogs || {})
        };
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          showHudToast('AI settings require the Windows app');
          return false;
        }
        try {
          await window.aetherBridge.saveRouterConfig(cfg);
          nativeRouterConfig = Object.assign({}, cfg, { litellm_api_key: '' });
          nativeHasLiteLlmKey = await window.aetherBridge.hasLiteLlmKey();
          var keyInput = document.getElementById('cfgLiteLlmKey');
          if (keyInput) keyInput.value = '';
          updateSidebarAiEngine();
          if (!quiet) showHudToast('AI settings saved securely');
          return true;
        } catch (err) {
          showHudToast('Save error: ' + err);
          return false;
        }
      }

      var mcpConnection = null;
      var mcpAuthorization = null;
      var mcpBusy = false;
      async function handleMcpSettingsAction(action) {
        if (mcpBusy) return;
        if (!window.aetherBridge?.isNative) { showHudToast('MCP requires the desktop app.'); return; }
        var resultEl = document.getElementById('cfgMcpResult');
        var select = document.getElementById('cfgMcpTool');
        var endpoint = document.getElementById('cfgMcpUrl')?.value.trim();
        var render = function(value) {
          if (resultEl) resultEl.textContent = typeof value === 'string' ? value : JSON.stringify(value,null,2);
        };
        mcpBusy = true;
        try {
          if (action === 'mcp-oauth-begin') {
            var issuer = document.getElementById('cfgMcpIssuer')?.value.trim();
            var clientId = document.getElementById('cfgMcpClientId')?.value.trim();
            var scope = document.getElementById('cfgMcpScope')?.value.trim() || '';
            if (!window.confirm('Authorize this trusted MCP resource using the specified issuer?\n'+endpoint+'\n'+issuer)) return;
            mcpAuthorization = null;
            render('Checking authorization metadata...');
            var flow = await window.aetherBridge.beginMcpOAuth(endpoint,issuer,clientId,scope);
            mcpAuthorization = {id:flow.id,url:flow.authorizationUrl,endpoint:endpoint};
            render('Copy the authorization URL, open it in your regular browser, and approve only the intended access. Then select Finish authorization.\n\n'+flow.authorizationUrl+'\n\nLoopback redirect: '+flow.redirectUri);
          } else if (action === 'mcp-oauth-copy') {
            if (!mcpAuthorization) throw new Error('Start authorization first.');
            await navigator.clipboard.writeText(mcpAuthorization.url);
            showHudToast('Authorization URL copied.');
          } else if (action === 'mcp-oauth-finish') {
            if (!mcpAuthorization || mcpAuthorization.endpoint !== endpoint) throw new Error('Start authorization for this endpoint first.');
            var id = mcpAuthorization.id;
            mcpAuthorization = null;
            render('Waiting for the reviewed OAuth callback...');
            await window.aetherBridge.finishMcpOAuth(id);
            render('Authorization saved securely. Select Discover tools.');
          } else if (action === 'mcp-oauth-forget') {
            if (!window.confirm('Forget the saved authorization for this exact MCP endpoint?')) return;
            if (mcpConnection && mcpConnection.endpoint === endpoint) {
              await window.aetherBridge.disconnectMcp(mcpConnection.id);
              mcpConnection = null;
              if (select) select.replaceChildren(new Option('Connect first',''));
            }
            await window.aetherBridge.forgetMcpOAuth(endpoint);
            mcpAuthorization = null;
            render('Local authorization removed. Revoke the grant at the provider if necessary.');
          } else if (action === 'mcp-marketplace') {
            var query = document.getElementById('cfgMcpSearch')?.value.trim() || '';
            render('Searching official MCP registry...');
            render(await window.aetherBridge.searchMcpMarketplace(query));
          } else if (action === 'mcp-connect') {
            if (mcpConnection) await window.aetherBridge.disconnectMcp(mcpConnection.id);
            mcpConnection = null;
            if (select) select.replaceChildren(new Option('Discovering...',''));
            render('Discovering tools...');
            var found = await window.aetherBridge.connectMcp(endpoint);
            mcpConnection = {id:found.id, endpoint:endpoint, tools:found.tools};
            if (select) {
              select.replaceChildren(new Option('Select a tool',''));
              found.tools.forEach(function(tool) { select.append(new Option(tool.name,tool.name)); });
            }
            render({protocol:found.version, tools:found.tools});
          } else if (action === 'mcp-lucia-enable' || action === 'mcp-lucia-revoke') {
            if (!mcpConnection || endpoint !== mcpConnection.endpoint) throw new Error('Discover tools for this exact endpoint first.');
            var alias = document.getElementById('cfgMcpAlias')?.value.trim();
            var name = select?.value;
            var enable = action === 'mcp-lucia-enable';
            var tool = mcpConnection.tools.find(function(t){return t.name===name;});
            if (!tool || !alias) throw new Error('Enter a server name and select a discovered tool.');
            if (enable && (!tool.annotations?.readOnlyHint || tool.annotations?.destructiveHint === true)) throw new Error('Only reviewed read-only tools are eligible.');
            if (!window.confirm((enable?'Approve this read-only tool for explicit Lucia MCP tasks?':'Revoke this tool from future Lucia tasks?')+'\n'+endpoint+'\n'+alias+'/'+name)) return;
            await window.aetherBridge.setLuciaMcpTool(mcpConnection.id,alias,name,enable);
            render(enable?'Approved. Ask Lucia: mcp auto '+alias+' [your read-only task]':'Revoked for future Lucia tasks.');
          } else if (action === 'mcp-disconnect') {
            if (mcpConnection) await window.aetherBridge.disconnectMcp(mcpConnection.id);
            mcpConnection = null;
            if (select) select.replaceChildren(new Option('Connect first',''));
            render('Disconnected.');
          } else {
            if (!mcpConnection || endpoint !== mcpConnection.endpoint) throw new Error('Discover tools for this endpoint first.');
            var name = select?.value;
            if (!mcpConnection.tools.some(function(t){return t.name===name;})) throw new Error('Select a discovered tool.');
            var args = JSON.parse(document.getElementById('cfgMcpArgs')?.value || '{}');
            if (!args || Array.isArray(args) || typeof args !== 'object') throw new Error('Arguments must be a JSON object.');
            if (!window.confirm('Send this exact MCP call to the trusted server?\n'+endpoint+'\n'+name+'\n'+JSON.stringify(args,null,2))) {
              render('Cancelled. No tool call sent.'); return;
            }
            render('Calling '+name+'...');
            render(await window.aetherBridge.callMcp(mcpConnection.id,name,args));
          }
        } catch (err) { render('MCP error: '+String(err)); }
        finally {mcpBusy=false;}
      }

      function showModelTestStatus(message) {
        nativeModelTestStatus = message;
        var statusEl = document.getElementById('cfgConnectionStatus');
        if (statusEl) statusEl.textContent = message;
        var indicator = document.getElementById('cfgConnectionIndicator');
        if (indicator) {
          indicator.dataset.state = message.indexOf('Connected:') === 0 ? 'success' :
            message.indexOf('Connection failed:') === 0 ? 'error' :
            message.indexOf('Contacting') === 0 ? 'testing' : 'idle';
        }
      }

      function showCloudFetchStatus(message) {
        nativeCloudFetchStatus = message;
        var status = document.getElementById('cfgFetchModelsStatus');
        if (status) status.textContent = message;
      }

      async function fetchNativeCloudModels() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          showCloudFetchStatus('Model discovery requires the Windows app.');
          return;
        }
        var button = document.querySelector('[data-action="fetch-cloud-models"]');
        if (button && button.disabled) return;
        if (button) {
          button.disabled = true;
          button.textContent = 'Fetching…';
        }
        showCloudFetchStatus('Contacting the provider’s models API…');
        try {
          // Save the current endpoint and API key first, keeping the key out of the model catalog.
          var saved = await saveNativeRouterSettings(true);
          if (!saved) throw new Error('Could not save the endpoint or API key.');
          var result = await window.aetherBridge.fetchCloudModels();
          if (!result || !Array.isArray(result.models) || !result.models.length) {
            throw new Error('The provider returned no model IDs.');
          }
          // The Rust backend has already saved this catalog in SQLite.
          nativeRouterConfig.cloud_model_catalogs = Object.assign(
            {}, nativeRouterConfig.cloud_model_catalogs || {},
            { [result.endpoint]: result.models }
          );
          nativeRouterConfig.litellm_url = result.endpoint;
          nativeRouterConfig.litellm_model = result.selected_model;
          nativeHasLiteLlmKey = await window.aetherBridge.hasLiteLlmKey();
          updateSidebarAiEngine();
          if (activeSettingsPane === 'ai') renderSettingsPane();
          showCloudFetchStatus(result.models.length + ' models fetched and saved. Select a model below to activate it.');
          showModelTestStatus('Fetched ' + result.models.length + ' model IDs from this provider. Run Test cloud connection to verify the selected model.');
          showHudToast(result.models.length + ' cloud models saved');
        } catch (err) {
          var detail = String(err);
          showCloudFetchStatus('Fetch failed: ' + detail);
          showModelTestStatus('Connection failed: ' + detail);
        } finally {
          var currentButton = document.querySelector('[data-action="fetch-cloud-models"]');
          if (currentButton) {
            currentButton.disabled = false;
            currentButton.textContent = 'Fetch Models';
          }
        }
      }

      async function testNativeModelConnection(backend) {
        var saved = await saveNativeRouterSettings();
        if (!saved) return;
        showModelTestStatus(backend === 'cloud-agent' ? 'Checking cloud model browser-action JSON...' :
          backend === 'local' ? 'Contacting Ollama...' :
          backend === 'cloud' ? 'Contacting cloud provider...' : 'Contacting selected model...');
        try {
          var result = await window.aetherBridge.testModelConnection(backend);
          showModelTestStatus('Connected: ' + result);
          showHudToast('Model connection verified');
        } catch (err) {
          showModelTestStatus('Connection failed: ' + err);
          showHudToast('Model test failed: ' + err);
        }
      }

      async function removeNativeModelKey() {
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          showHudToast('Requires Windows app');
          return;
        }
        try {
          await window.aetherBridge.removeModelApiKey();
          nativeHasLiteLlmKey = false;
          renderSettingsPane();
          showHudToast('Provider API key removed');
        } catch (err) {
          showHudToast('Key removal failed: ' + err);
        }
      }

      function saveNewNativeCredential() {
        var site = (document.getElementById('credNewSite')?.value || '').trim();
        var user = (document.getElementById('credNewUser')?.value || '').trim();
        var pass = (document.getElementById('credNewPass')?.value || '');
        if (!site || !user || !pass) {
          showHudToast('Enter website, username, and password');
          return;
        }
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.saveCredential(site, user, pass).then(function () {
            hydrateCredentialsFromNative();
            showHudToast('Saved to Windows Credential Manager');
          }).catch(function (err) {
            showHudToast('Credential error: ' + err);
          });
        } else {
          browserSettings.savedLogins.unshift({ site: site, username: user, masked: '••••••••••••', plain: pass });
          renderSettingsPane();
        }
      }

      function deleteNativeCredential(site, username) {
        if (!site || !username) return;
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.deleteCredential(site, username).then(function () {
            delete nativeRevealedCache[site + '::' + username];
            delete revealedPasswords[site + '::' + username];
            hydrateCredentialsFromNative();
            showHudToast('Credential removed');
          }).catch(function () {});
        }
      }

      function revealNativeOrDemoPassword(site, username) {
        var key = site + '::' + username;
        if (revealedPasswords[key]) {
          revealedPasswords[key] = false;
          renderSettingsPane();
          return;
        }
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.getCredentialSecret(site, username).then(function (secret) {
            nativeRevealedCache[key] = secret || '(not found)';
            revealedPasswords[key] = true;
            renderSettingsPane();
          }).catch(function () {});
        } else {
          revealedPasswords[key] = true;
          renderSettingsPane();
        }
      }

      function copyNativeOrDemoPassword(site, username, fallbackSecret) {
        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.getCredentialSecret(site, username).then(function (secret) {
            if (secret) {
              copyTextToClipboard(secret);
              showHudToast('Password copied from Windows Credential Manager');
            }
          }).catch(function () {});
        } else {
          copyTextToClipboard(fallbackSecret || '');
          showHudToast('Password copied to clipboard');
        }
      }


      function buildStepObjectFromInputs() {
        var kind = document.getElementById('autoStepKind')?.value || 'http_get';
        var target = (document.getElementById('autoStepTarget')?.value || '').trim();
        var payload = (document.getElementById('autoStepPayload')?.value || '').trim();

        if (kind === 'browser_nav') {
          var u = target || 'https://example.org';
          if (u.indexOf('http://') !== 0 && u.indexOf('https://') !== 0) u = 'https://' + u;
          var doms = payload ? payload.split(',').map(function (s) { return s.trim(); }).filter(Boolean) : [];
          return { kind: 'browser', action: { type: 'navigate', url: u }, allowed_domains: doms };
        }
        if (kind === 'local_write') {
          return { kind: 'local_write_file', filename: target || 'output.txt', content: payload || 'Generated by Browsphere Automation Hub', append: false };
        }
        if (kind === 'local_cmd') {
          return { kind: 'local_command', command: target || 'echo Browsphere OK', working_dir: payload || null };
        }
        if (kind === 'llm_prompt') {
          return { kind: 'llm_prompt', prompt: target || 'Summarize active page context', origin_hint: payload || null };
        }
        return { kind: 'http_request', method: 'GET', url: target || 'https://httpbin.org/get', headers: {}, body: payload || null };
      }

      function summarizeStep(step) {
        if (!step || !step.kind) return 'Step';
        if (step.kind === 'http_request') return 'HTTP ' + (step.method || 'GET') + ' ' + (step.url || '');
        if (step.kind === 'browser') return 'Browser ' + ((step.action && step.action.type) || 'action') + ' ' + ((step.action && step.action.url) || '');
        if (step.kind === 'local_write_file') return 'Write ' + (step.filename || 'file');
        if (step.kind === 'local_command') return 'Cmd: ' + (step.command || '');
        if (step.kind === 'llm_prompt') return 'AI: ' + (step.prompt || '');
        return step.kind;
      }

      function renderDraftAutomationSteps() {
        var list = document.getElementById('autoDraftStepsList');
        if (!list) return;
        if (!draftAutomationSteps.length) {
          list.innerHTML = '<div class="text-[10.5px] opacity-55">Chain queue empty — click "+ Add Step" to stack steps (or click Save Chain to save the current step).</div>';
          return;
        }
        list.innerHTML = draftAutomationSteps.map(function (st, i) {
          return (
            '<div class="flex items-center justify-between gap-1.5 px-2 py-1 rounded bg-white/80 dark:bg-zinc-900/80 border border-black/10 dark:border-white/10 text-[10.5px]">' +
              '<span class="truncate font-medium"><strong>' + (i + 1) + '.</strong> ' + esc(summarizeStep(st)) + '</span>' +
              '<div class="flex items-center gap-0.5 shrink-0">' +
                (i > 0 ? '<button type="button" data-action="move-draft-step" data-step-idx="' + i + '" data-delta="-1" class="px-1 rounded hover:bg-black/10">↑</button>' : '') +
                (i < draftAutomationSteps.length - 1 ? '<button type="button" data-action="move-draft-step" data-step-idx="' + i + '" data-delta="1" class="px-1 rounded hover:bg-black/10">↓</button>' : '') +
                '<button type="button" data-action="remove-draft-step" data-step-idx="' + i + '" class="px-1 rounded text-rose-500 hover:bg-rose-500/10">✕</button>' +
              '</div>' +
            '</div>'
          );
        }).join('');
      }

      function appendDraftAutomationStep() {
        var stepObj = buildStepObjectFromInputs();
        draftAutomationSteps.push(stepObj);
        document.getElementById('autoStepTarget').value = '';
        document.getElementById('autoStepPayload').value = '';
        renderDraftAutomationSteps();
      }

      function moveDraftAutomationStep(idx, delta) {
        var targetIdx = idx + delta;
        if (idx < 0 || idx >= draftAutomationSteps.length || targetIdx < 0 || targetIdx >= draftAutomationSteps.length) return;
        var tmp = draftAutomationSteps[idx];
        draftAutomationSteps[idx] = draftAutomationSteps[targetIdx];
        draftAutomationSteps[targetIdx] = tmp;
        renderDraftAutomationSteps();
      }

      function loadAutomationIntoBuilder(autoId) {
        var found = nativeAutomations.find(function (j) { return j.id === autoId; });
        if (!found) return;
        editingAutomationId = found.id;
        document.getElementById('autoJobName').value = found.name || '';
        document.getElementById('autoJobTrigger').value = found.trigger || 'manual';
        document.getElementById('autoJobIntervalSecs').value = String(found.interval_secs || 300);
        draftAutomationSteps = Array.isArray(found.steps) ? found.steps.slice() : [];
        renderDraftAutomationSteps();
        showHudToast('Loaded "' + found.name + '" into chain editor');
      }

      function createNativeAutomationFromUi() {
        var name = (document.getElementById('autoJobName')?.value || '').trim() || 'Multi-Step Workflow';
        var trig = document.getElementById('autoJobTrigger')?.value || 'manual';
        var rawSecs = parseInt(document.getElementById('autoJobIntervalSecs')?.value || '300', 10);
        var intervalSecs = trig === 'interval' ? Math.max(30, isNaN(rawSecs) ? 300 : rawSecs) : null;

        var stepsToSave = draftAutomationSteps.slice();
        var typedTarget = (document.getElementById('autoStepTarget')?.value || '').trim();
        var typedPayload = (document.getElementById('autoStepPayload')?.value || '').trim();
        if (!stepsToSave.length || typedTarget || typedPayload) {
          stepsToSave.push(buildStepObjectFromInputs());
        }

        var jobSpec = {
          id: editingAutomationId,
          name: name,
          trigger: trig,
          interval_secs: intervalSecs,
          enabled: true,
          steps: stepsToSave
        };

        if (window.aetherBridge && window.aetherBridge.isNative) {
          window.aetherBridge.saveAutomationJob(jobSpec).then(function () {
            editingAutomationId = null;
            draftAutomationSteps = [];
            document.getElementById('autoJobName').value = '';
            document.getElementById('autoStepTarget').value = '';
            document.getElementById('autoStepPayload').value = '';
            renderDraftAutomationSteps();
            showHudToast('Saved automation "' + name + '" (' + stepsToSave.length + ' steps)');
          }).catch(function (err) {
            showHudToast('Save error: ' + err);
          });
        } else {
          nativeAutomations.unshift({
            id: editingAutomationId || ('local-' + Date.now()),
            name: name,
            trigger: trig,
            interval_secs: intervalSecs,
            enabled: true,
            steps: stepsToSave,
            last_status: 'idle',
            run_count: 0
          });
          editingAutomationId = null;
          draftAutomationSteps = [];
          renderDraftAutomationSteps();
          renderAutomationsList();
        }
      }

      function renderAutomationsList() {
        renderDraftAutomationSteps();
        var container = document.getElementById('automationsListContainer');
        if (!container) return;
        if (!nativeAutomations.length) {
          container.innerHTML = '<div class="p-4 text-center opacity-55 text-[11.5px]">No saved workflows yet. Stack steps with "+ Add Step" or save a chain above.</div>';
          return;
        }
        container.innerHTML = nativeAutomations.map(function (job) {
          var stepList = Array.isArray(job.steps) ? job.steps : [];
          var stepCount = stepList.length;
          var trigLabel = job.trigger === 'interval' ? ('Every ' + (job.interval_secs || 300) + 's') : 'Manual';
          var chainPreview = stepList.map(function (s, idx) {
            return (idx + 1) + '. ' + esc(summarizeStep(s));
          }).join(' → ');
          return (
            '<div class="p-2.5 rounded-xl bg-black/[0.03] dark:bg-white/[0.05] border border-black/10 dark:border-white/10 space-y-1.5">' +
              '<div class="flex items-center justify-between gap-2">' +
                '<div class="min-w-0">' +
                  '<div class="font-semibold truncate text-[12px]">' + esc(job.name) + '</div>' +
                  '<div class="text-[10px] opacity-60">' + esc(trigLabel) + ' · ' + stepCount + ' step(s) · Status: ' + esc(job.last_status || 'idle') + ' (' + esc(job.run_count || 0) + ' runs)</div>' +
                '</div>' +
                '<div class="flex items-center gap-1 shrink-0">' +
                  '<button type="button" data-action="run-automation" data-auto-id="' + esc(job.id) + '" class="px-2 py-0.5 rounded bg-[#007AFF] text-white text-[10.5px] font-semibold hover:brightness-110">Run</button>' +
                  '<button type="button" data-action="edit-automation" data-auto-id="' + esc(job.id) + '" class="px-1.5 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10.5px]">Edit</button>' +
                  '<button type="button" data-action="stop-automation" data-auto-id="' + esc(job.id) + '" class="px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-600 dark:text-amber-400 text-[10.5px] font-medium">Stop</button>' +
                  (job.trigger === 'interval'
                    ? '<button type="button" data-action="toggle-automation-enabled" data-auto-id="' + esc(job.id) + '" data-enabled="' + (job.enabled ? 'true' : 'false') + '" class="px-1.5 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10.5px]">' + (job.enabled ? 'On' : 'Off') + '</button>'
                    : '') +
                  '<button type="button" data-action="delete-automation" data-auto-id="' + esc(job.id) + '" class="px-1.5 py-0.5 rounded bg-rose-500/15 text-rose-500 text-[10.5px]">✕</button>' +
                '</div>' +
              '</div>' +
              (chainPreview ? '<div class="text-[10px] font-mono opacity-65 truncate">' + chainPreview + '</div>' : '') +
            '</div>'
          );
        }).join('');
      }

      function hydrateAutomationsFromNative() {
        renderDraftAutomationSteps();
        if (!window.aetherBridge || !window.aetherBridge.isNative) {
          renderAutomationsList();
          return;
        }
        window.aetherBridge.listAutomations(40).then(function (rows) {
          if (Array.isArray(rows)) nativeAutomations = rows;
          renderAutomationsList();
        }).catch(function () {});
      }

      function handleLiveDownloadProgressEvent(ev) {
        var rec = ev && ev.detail;
        if (!rec || !rec.id) return;
        var idx = downloads.findIndex(function (item) { return item.id === rec.id; });
        if (idx >= 0) {
          downloads[idx] = Object.assign({}, downloads[idx], rec);
        } else {
          downloads.unshift(rec);
        }
        renderDownloadsList();
      }

      function bindNativeHydrationEvents() {
        window.addEventListener('aether:history-updated', hydrateHistoryFromNative);
        window.addEventListener('aether:download-progress', handleLiveDownloadProgressEvent);
        window.addEventListener('aether:downloads-updated', hydrateDownloadsFromNative);
        window.addEventListener('aether:itp-updated', hydrateItpFromNative);
        window.addEventListener('aether:traces-updated', hydrateTracesFromNative);
        ['plan','stage','activity','trace','approval','status','error','verification','review','vision','supervisor','orchestration','workflow_graph'].forEach(function (type) {
          window.addEventListener('aether:agent-' + type, function (event) {
            applyLuciaAgentEvent(type, event.detail || {});
          });
        });
        window.addEventListener('aether:macros-updated', hydrateMacrosFromNative);
        window.addEventListener('aether:automations-updated', hydrateAutomationsFromNative);
        window.addEventListener('aether:open-url-from-chat', function (ev) {
          var d = ev && ev.detail;
          if (!d || !d.url) return;
          if (d.new_tab) {
            createNewTab(d.url);
          } else {
            navigateToUrl(d.url);
          }
        });
      }

      function getPreviewMeta(url) {
        var clean = String(url || '').replace(/^https?:\/\//, '');
        var newsMatch = newTabNewsItems.find(function (n) { return n.url === clean; });
        if (newsMatch) return newsMatch;
        var page = getPageData(clean);
        var domain = clean.split('/')[0] || 'Browsphere.local';
        var firstBullet = (page.summaryBullets && page.summaryBullets[0])
          ? page.summaryBullets[0].replace(/<[^>]+>/g, '')
          : 'Indexed locally in Browsphere workspace.';
        return {
          category: page.group || 'Web Page',
          source: domain,
          title: page.title || domain,
          dek: page.critique || firstBullet,
          verifiedQuote: firstBullet,
          readTime: page.readTime || '4 min read',
          timeAgo: 'Live',
          url: clean,
          thumbSvg: '<svg viewBox="0 0 120 72" class="w-full h-full"><rect width="120" height="72" fill="#F8FAFC"/><rect x="10" y="10" width="100" height="9" rx="3" fill="#E2E8F0"/><rect x="10" y="25" width="70" height="6" rx="2" fill="#007AFF" opacity="0.8"/><rect x="10" y="36" width="96" height="4" rx="1.5" fill="#CBD5E1"/><rect x="10" y="45" width="84" height="4" rx="1.5" fill="#CBD5E1"/><rect x="10" y="54" width="60" height="4" rx="1.5" fill="#E2E8F0"/></svg>'
        };
      }

      function getMatchingSourcesForPrompt(promptText, activeUrl) {
        var lower = String(promptText || '').toLowerCase();
        var picks = [];
        if (activeUrl && activeUrl.indexOf('browsphere://') !== 0 && activeUrl !== 'about:blank') {
          picks.push(activeUrl.replace(/^https?:\/\//, ''));
        }
        newTabNewsItems.forEach(function (item) {
          var hay = (item.title + ' ' + item.dek + ' ' + item.category + ' ' + item.source).toLowerCase();
          var words = lower.split(/\s+/).filter(function (w) { return w.length > 3; });
          var matched = words.some(function (w) { return hay.indexOf(w) !== -1; });
          if (matched && picks.indexOf(item.url) === -1) picks.push(item.url);
        });
        if (!picks.length) {
          picks.push('arxiv-review.org/quantum-2026', 'ml-explore.github.io/mlx');
        }
        return picks.slice(0, 3);
      }

      function buildSourceChipsHtml(urls) {
        return (urls || []).map(function (u) {
          var meta = getPreviewMeta(u);
          return (
            '<button type="button" data-action="navigate" data-url="' + esc(meta.url) + '" data-preview-url="' + esc(meta.url) + '" class="qp-source-chip">' +
              '<span class="qp-mini-thumb">' + meta.thumbSvg + '</span>' +
              '<span class="truncate max-w-[130px]">' + esc(meta.source) + '</span>' +
            '</button>'
          );
        }).join('');
      }

      function showQuickPreview(anchorEl, url) {
        var card = document.getElementById('quickPreviewCard');
        if (!card || !anchorEl || !url) return;
        var meta = getPreviewMeta(url);
        qpActiveUrl = meta.url;

        document.getElementById('qpDomain').textContent = meta.url;
        document.getElementById('qpMeta').textContent = meta.readTime + (meta.timeAgo ? ' · ' + meta.timeAgo : '');
        document.getElementById('qpScreenFrame').innerHTML = meta.thumbSvg;
        document.getElementById('qpCategory').textContent = meta.category + ' — ' + meta.source;
        document.getElementById('qpTitle').textContent = meta.title;
        document.getElementById('qpExcerpt').textContent = meta.dek;
        document.getElementById('qpVerifiedBox').textContent = meta.verifiedQuote;

        document.getElementById('qpOpenBtn').dataset.url = meta.url;
        document.getElementById('qpNewTabBtn').dataset.url = meta.url;
        document.getElementById('qpAskBtn').dataset.url = meta.url;

        var rect = anchorEl.getBoundingClientRect();
        var cardW = 356;
        var cardH = 330;
        var margin = 12;

        // Prefer popping out to the right or left of wide cards, or above/below compact chips
        var left = rect.right + 12;
        var top = rect.top - 8;

        if (left + cardW > window.innerWidth - margin) {
          left = rect.left - cardW - 12;
        }
        if (left < margin) {
          left = Math.min(window.innerWidth - cardW - margin, Math.max(margin, rect.left));
          top = rect.top - cardH - 10;
          if (top < margin) {
            top = Math.min(window.innerHeight - cardH - margin, rect.bottom + 10);
          }
        }
        if (top + cardH > window.innerHeight - margin) {
          top = Math.max(margin, window.innerHeight - cardH - margin);
        }
        if (top < margin) top = margin;

        card.style.left = Math.round(left) + 'px';
        card.style.top = Math.round(top) + 'px';
        card.classList.add('qp-open');

        if (window.aetherBridge && window.aetherBridge.isNative && typeof window.aetherBridge.fetchUrlPreview === 'function') {
          var targetUrlForFetch = meta.url;
          window.aetherBridge.fetchUrlPreview(targetUrlForFetch).then(function (live) {
            if (!live || qpActiveUrl !== targetUrlForFetch) return;
            if (live.title) document.getElementById('qpTitle').textContent = live.title;
            if (live.description) document.getElementById('qpExcerpt').textContent = live.description;
            if (live.excerpt) document.getElementById('qpVerifiedBox').textContent = live.excerpt;
            if (live.image_url) {
              document.getElementById('qpScreenFrame').innerHTML =
                '<img src="' + esc(live.image_url) + '" alt="" style="width:100%;height:100%;object-fit:cover;display:block;" />';
            }
          }).catch(function () {});
        }
      }

      function hideQuickPreview(immediate) {
        clearTimeout(qpHoverTimer);
        clearTimeout(qpHideTimer);
        var card = document.getElementById('quickPreviewCard');
        if (!card) return;
        if (immediate) {
          card.classList.remove('qp-open');
          qpActiveUrl = '';
        } else {
          qpHideTimer = setTimeout(function () {
            card.classList.remove('qp-open');
            qpActiveUrl = '';
          }, 190);
        }
      }

      function setupQuickPreviewListeners() {
        var card = document.getElementById('quickPreviewCard');
        if (!card) return;

        document.addEventListener('pointerover', function (e) {
          var trigger = e.target.closest('[data-preview-url]');
          if (!trigger) return;
          var url = trigger.dataset.previewUrl;
          if (!url) return;
          clearTimeout(qpHideTimer);
          clearTimeout(qpHoverTimer);
          qpHoverTimer = setTimeout(function () {
            showQuickPreview(trigger, url);
          }, 130);
        });

        document.addEventListener('pointerout', function (e) {
          var trigger = e.target.closest('[data-preview-url]');
          if (!trigger) return;
          var related = e.relatedTarget;
          if (related && (trigger.contains(related) || card.contains(related))) return;
          hideQuickPreview(false);
        });

        card.addEventListener('pointerenter', function () {
          clearTimeout(qpHideTimer);
        });

        card.addEventListener('pointerleave', function (e) {
          var related = e.relatedTarget;
          if (related && related.closest && related.closest('[data-preview-url]')) return;
          hideQuickPreview(false);
        });

        var canvas = document.getElementById('readingCanvas');
        if (canvas) {
          canvas.addEventListener('scroll', function () {
            hideQuickPreview(true);
          }, { passive: true });
        }
      }

      function toggleNewTabHistoryDrawer(force) {
        var drawer = document.getElementById('newTabHistoryDrawer');
        if (!drawer) return;
        var shouldOpen = typeof force === 'boolean' ? force : drawer.classList.contains('drawer-closed');
        newTabHistoryOpen = shouldOpen;
        if (shouldOpen) {
          closeAllPopovers();
          hideQuickPreview(true);
          renderNewTabHistoryDrawer();
          drawer.classList.remove('drawer-closed');
        } else {
          drawer.classList.add('drawer-closed');
        }
      }

      function getActiveNewTabThread() {
        var id = getActiveTab()?.chatThreadId;
        return id ? newTabChatHistory.find(function (thread) { return thread.id === id; }) || null : null;
      }

      function renderNewTabConversation() {
        var thread = getActiveNewTabThread();
        if (!thread) return PRESET_PAGES['browsphere://start'].renderHtml();
        var messagesHtml = thread.messages.map(function (message) {
          if (message.role === 'user') {
            return '<div class="main-chat-entry user"><div class="main-chat-user-bubble">' +
              esc(message.content) + '</div></div>';
          }
          return '<div class="main-chat-entry assistant"><div class="main-chat-assistant">' +
            '<div class="main-chat-answer">' +
            '<h3>Lucia</h3>' +
            '<div class="main-chat-answer-body' + (message.error ? ' is-error' : '') + '">' +
            esc(message.content) + '</div>' +
            (!message.error && message.model ? '<div class="main-chat-model">Answered by ' + esc(message.model) + '</div>' : '') +
            '</div></div></div>';
        }).join('');
        if (thread.agentTask) {
          var task = thread.agentTask;
          var stages = task.plan?.stages || [];
          var labels = {pending:'Pending', active:'Working', completed:'Done'};
          var liveLog = Array.isArray(task.activityLog) ? task.activityLog : [];
          messagesHtml += '<section class="main-chat-agent-progress" aria-label="Lucia task progress">' +
            '<div class="main-chat-agent-heading"><strong>Lucia · Specialist Task</strong>' +
            '<span class="main-chat-agent-status">' + esc(task.status) + '</span></div>' +
            '<div class="main-chat-agent-plan">' + stages.map(function (stage, index) {
              return '<div class="main-chat-agent-stage" data-state="' + esc(stage.status) + '">' +
                '<span class="main-chat-stage-index">' + (index + 1) + '</span>' +
                '<div class="main-chat-stage-name">' + esc(stage.specialist.replace(/_/g, ' ')) +
                '<small>' + esc(stage.objective) + '</small></div>' +
                '<span class="main-chat-stage-status">' + esc(labels[stage.status] || stage.status) + '</span></div>';
            }).join('') + '</div>' +
            (task.workflowGraph ? '<div class="main-chat-agent-plan" aria-label="Adaptive workflow dependencies"><strong>Workflow graph</strong>' +
              task.workflowGraph.nodes.map(function(node) {
                return '<div class="main-chat-agent-stage" data-state="' + esc(node.state) + '">' +
                  '<span class="main-chat-stage-index">' + (node.id+1) + '</span>' +
                  '<div class="main-chat-stage-name">' + esc(node.label) +
                  '<small>' + esc(node.depends_on.length ? 'Depends on '+node.depends_on.map(d=>d+1).join(', ') : 'No dependencies') + '</small></div>' +
                  '<span class="main-chat-stage-status">' + esc(node.state) + '</span></div>';
              }).join('')+'</div>' : '') +
            '<div class="main-chat-agent-activity" aria-label="Lucia live activity">' +
            '<div class="main-chat-activity-heading"><strong>Live activity</strong>' +
            '<small>Actions and decision summaries, not private internal reasoning</small></div>' +
            '<div class="main-chat-activity-current" role="status">' +
              (task.status === 'Running' ? '<span class="main-chat-activity-dot"></span>' : '') +
              esc(task.liveActivity || 'Preparing the browser task…') + '</div>' +
            '<ol class="main-chat-activity-log">' +
            liveLog.slice(-60).reverse().map(function (item) {
              return '<li class="main-chat-activity-item" data-result="' + esc(item.status || 'unknown') + '">' +
                '<span class="main-chat-activity-step">' + (item.step ? 'Step ' + item.step : 'Live') + '</span>' +
                '<div class="main-chat-activity-content"><strong>' + esc(item.label || '') + '</strong>' +
                (item.kind === 'action' ? '<small>' + esc(item.outcomeLabel || 'Not confirmed') + '</small>' : '') +
                '<p>' + esc(item.explanation || '') + '</p>' +
                (item.kind === 'action' ? '<span class="main-chat-activity-meta">' +
                esc([item.model, item.source, item.latency].filter(Boolean).join(' · ')) + '</span>' : '') +
                '</div></li>';
            }).join('') +
            (liveLog.length ? '' : '<li class="main-chat-activity-empty">Waiting for live page observations…</li>') +
            '</ol></div>' +
            '<div class="main-chat-agent-footer"><span>' + (task.actions || 0) +
            ' browser steps · ' + (task.evidence || 0) + ' observed sources</span>' +
            (task.tabId && !task.tabClosed && tabs.some(function (tab) { return tab.id === task.tabId; }) ?
              '<button type="button" class="main-chat-task-view" data-action="nt-view-task-tab" data-tab-id="' +
              esc(task.tabId) + '">View task tab</button>' : '') +
            (task.status === 'Running' ? '<button type="button" class="main-chat-task-stop" data-action="nt-task-stop">Stop task</button>' : '') +
            '</div>' +
            (task.verificationReport?.findings?.length ?
              '<div class="main-chat-evidence"><div class="main-chat-evidence-title">Evidence checked · ' +
              task.verificationReport.findings.length + ' sourced findings</div>' +
              task.verificationReport.findings.map(function (finding, i) {
                var safeUrl = /^https?:\/\//i.test(finding.url) ? finding.url : '';
                return '<div class="main-chat-evidence-item"><div class="main-chat-evidence-claim">' +
                  esc(finding.claim) + '</div>' +
                  (safeUrl ? '<a href="' + esc(safeUrl) + '" target="_blank" rel="noopener noreferrer">' +
                    esc(safeUrl) + '</a>' : '') +
                  '<blockquote>' + esc(finding.quote) + '</blockquote></div>';
              }).join('') + '</div>' : '') +
            (task.error ? '<p class="main-chat-agent-error">' + esc(task.error) + '</p>' : '') +
            (task.approval ? '<div class="main-chat-agent-approval"><strong>Approval required</strong><p>' +
                esc(task.approval.reason || 'An action needs your consent.') + '</p>' +
                '<div class="main-chat-agent-approval-actions">' +
                '<button type="button" data-action="nt-task-reject" data-approval-id="' + esc(task.approval.approval_id) + '">Reject</button>' +
                '<button type="button" data-action="nt-task-approve" data-approval-id="' + esc(task.approval.approval_id) + '">Approve once</button>' +
                '</div></div>' : '') +
            '</section>';
        }
        if (thread.pending && !thread.agentTask) {
          messagesHtml += '<div class="main-chat-entry assistant"><div class="main-chat-assistant">' +
            '' +
            '<div class="main-chat-answer"><h3>Lucia</h3>' +
            '<div class="main-chat-thinking" role="status"><span class="main-chat-thinking-dot"></span>Thinking…</div>' +
            '</div></div></div>';
        }
        return '<section class="main-chat-page" aria-label="Archived AI Conversation">' +
          '<div class="main-chat-stream" id="mainChatMessages" role="log" aria-live="polite">' +
          '<div class="main-chat-inner">' + messagesHtml + '</div></div>' +
          '<div class="main-chat-bottom">' +
          '<form id="newTabChatForm" class="main-chat-composer">' +
          '<textarea id="newTabChatInput" rows="1" ' +
          (thread.pending ? 'disabled ' : '') +
          'placeholder="Message Lucia…" aria-label="Message Lucia" autocomplete="off"></textarea>' +
          '<button type="button" class="main-chat-task-btn" data-action="nt-run-agent"' +
          (thread.pending ? ' disabled' : '') + '>Run task</button>' +
          '<button type="submit" class="main-chat-send" aria-label="Send message"' +
          (thread.pending ? ' disabled' : '') + '>↑</button>' +
          '</form><div class="main-chat-footnote">Responses come from your configured model. Sources are shown only when actually retrieved.</div>' +
          '</div></section>';
      }

      function renderNewTabHistoryDrawer() {
        var list = document.getElementById('ntHistoryList');
        var badge = document.getElementById('ntHistoryCountBadge');
        if (badge) badge.textContent = String(newTabChatHistory.length);
        if (!list) return;
        if (!newTabChatHistory.length) {
          list.innerHTML = '<div class="py-10 px-3 text-center text-[12.5px] opacity-55">No conversations yet. Ask a question to start one.</div>';
          return;
        }
        list.innerHTML = newTabChatHistory.map(function (thread) {
          var selected = getActiveNewTabThread()?.id === thread.id;
          return '<button type="button" data-action="nt-load-history" data-hid="' + esc(thread.id) +
            '" class="w-full text-left p-3 rounded-[11px] transition block ' +
            (selected ? 'bg-[#007AFF]/10 text-[#007AFF]' : 'hover:bg-black/[0.045] dark:hover:bg-white/[0.07]') + '">' +
            '<div class="text-[12.5px] font-medium leading-snug line-clamp-2">' + esc(thread.title) + '</div>' +
            '<div class="text-[11px] opacity-55 mt-1">' + esc(thread.time) + ' · ' + (thread.messages.length > 1 ? 'Conversation' : 'New') + '</div>' +
            '</button>';
        }).join('');
      }

      function loadNewTabHistoryItem(id) {
        var found = newTabChatHistory.find(function (thread) { return thread.id === id; });
        if (!found) return;
        if (getActiveTab().history[getActiveTab().historyIndex] !== 'browsphere://start') {
          navigateToUrl('browsphere://start');
        }
        getActiveTab().chatThreadId = found.id;
        getActiveTab().viewingArchivedChat = true;
        toggleNewTabHistoryDrawer(false);
        renderActiveTab();
      }

      function createLuciaThread(text) {
        var tab = getActiveTab();
        var thread = getActiveNewTabThread();
        if (thread) return thread;
        thread = {
          id: 'nt-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
          title: text.length > 65 ? text.slice(0, 65) + '…' : text,
          messages: [],
          time: new Date().toLocaleTimeString([], {hour:'2-digit', minute:'2-digit'}),
          pending: false
        };
        newTabChatHistory.unshift(thread);
        if (newTabChatHistory.length > 30) newTabChatHistory.pop();
        tab.chatThreadId = thread.id;
        tab.viewingArchivedChat = true;
        return thread;
      }

      function luciaRunId(payload) {
        return String(payload && payload.run_id || '');
      }

      function applyLuciaAgentEvent(kind, payload) {
        var thread = newTabChatHistory.find(function (item) {
          return item.agentTask && String(item.agentTask.runId) === luciaRunId(payload);
        });
        if (!thread) {
          if (pendingLuciaEvents.length < 60) pendingLuciaEvents.push({ kind:kind, payload:payload });
          return;
        }
        var task = thread.agentTask;
        if (kind === 'plan' && payload.plan) {
          task.plan = payload.plan;
        } else if (kind === 'stage') {
          var stage = task.plan?.stages?.[payload.index];
          if (stage) stage.status = payload.status;
          if (payload.evidence_count != null) task.evidence = payload.evidence_count;
        } else if (kind === 'verification' && payload.report?.status === 'evidence_checked') {
          task.verificationReport = payload.report;
          task.verificationSummary = payload.report.summary;
        } else if (kind === 'workflow_graph') {
          const graph=window.BrowsphereLuciaActivity?.workflowGraph(payload);
          if (graph && (!task.workflowGraph || graph.revision>=task.workflowGraph.revision)) task.workflowGraph=graph;
        } else if (kind === 'activity') {
          window.BrowsphereLuciaActivity?.append(
            task, window.BrowsphereLuciaActivity.activity(payload));
        } else if (['review','vision','supervisor','orchestration'].includes(kind)) {
          window.BrowsphereLuciaActivity?.append(
            task, window.BrowsphereLuciaActivity.specialist(kind,payload));
        } else if (kind === 'trace') {
          task.actions = (task.actions || 0) + 1;
          window.BrowsphereLuciaActivity?.append(
            task, window.BrowsphereLuciaActivity.trace(payload));
        } else if (kind === 'approval') {
          task.approval = payload;
        } else if (kind === 'error') {
          task.error = payload.message || 'The agent could not complete this step.';
        } else if (kind === 'status') {
          if (task.workflowGraph && payload.status !== 'completed') {
            for (const node of task.workflowGraph.nodes) {
              if (node.state === 'active' || node.state === 'pending') node.state = payload.status === 'cancelled' ? 'cancelled' : 'blocked';
            }
          }
          task.status = payload.status === 'completed' && task.verificationReport ? 'Completed' :
            (payload.status === 'cancelled' || payload.status === 'tab_closed') ? 'Stopped' : 'Not verified';
          task.approval = null;
          thread.pending = false;
          // A completed run is not a factual report on its own. The verification
          // specialist's last accepted summary is the best available result.
          var stages = task.plan?.stages || [];
          if (payload.status === 'completed' && task.verificationReport) {
            var completion = task.verificationReport.summary;
            thread.messages.push({role:'assistant',content:completion,model:'Lucia · Source-checked findings'});
          } else if (payload.status === 'tab_closed') {
            thread.messages.push({role:'assistant',content:task.error || 'Lucia stopped because its assigned task tab was closed.'});
          } else if (payload.status === 'tab_missing') {
            thread.messages.push({role:'assistant',content:task.error || 'Lucia lost its task WebView unexpectedly. Restart the task in a fresh tab.',error:true});
          } else if (payload.status === 'completed') {
            thread.messages.push({role:'assistant',content:'The task ended, but no source-backed verification report was delivered. Do not treat this as a verified result.',error:true});
          } else {
            thread.messages.push({role:'assistant',content:
              'The task did not finish (' + payload.status + '). ' +
              (task.error || ({
                halted:'Lucia stopped after repeated actions or an unrecovered browser failure. Inspect the task tab and its last action.',
                max_steps:'Lucia used its available steps without verifying the result. Inspect the task tab before retrying.',
                unverified:'Lucia could not verify that the webpage changed as requested. Inspect the destination before retrying.',
                rejected:'The requested browser action was blocked or not approved. No further actions were performed.',
                failed:'Lucia encountered an execution or model error. Review the latest task step and AI & ML model settings.'
              }[payload.status] || 'The task stopped before verification. Review the specialist progress.')),error:true});
          }
        }
        if (kind === 'stage' && payload.status === 'completed' &&
            /Verification Agent/i.test(payload.specialist || '')) {
          task.verificationSummary = payload.summary || '';
        }
        if (getActiveNewTabThread()?.id === thread.id) {
          renderActiveTab();
          scrollMainTabChatToBottom();
        }
      }

      function flushLuciaAgentEvents() {
        var pending = pendingLuciaEvents.splice(0);
        pending.forEach(function (event) {
          applyLuciaAgentEvent(event.kind, event.payload);
        });
      }

      async function answerLuciaTaskApproval(approvalId, approved) {
        try {
          if (!window.aetherBridge?.isNative) throw new Error('Requires Browsphere Windows app');
          await window.aetherBridge.resolveLuciaTaskApproval(approvalId, approved);
          newTabChatHistory.forEach(function (thread) {
            if (thread.agentTask?.approval?.approval_id === approvalId) {
              thread.agentTask.approval = null;
            }
          });
          renderActiveTab();
        } catch (err) {
          showHudToast('Approval error: ' + String(err));
        }
      }

      async function startNewTabAgentTask(overrideText) {
        var input = document.getElementById('newTabChatInput');
        var text = typeof overrideText === 'string' ? overrideText.trim() :
          (input ? input.value.trim() : '');
        if (!text) return;
        if (text.length > 2000) {
          showHudToast('Task is too long (2,000 character limit)');
          return;
        }
        var thread = createLuciaThread(text);
        if (thread.pending) return;
        thread.messages.push({role:'user',content:text});
        thread.pending = true;
        thread.agentTask = {
          runId: null, status:'Running', plan:{stages:[]},
          actions:0, evidence:0, approval:null, error:'', tabId:null, tabClosed:false,
          activityLog:[], liveActivity:'Starting the browser task…'
        };
        if (input) input.value = '';
        renderActiveTab();
        try {
          if (!window.aetherBridge?.isNative ||
              typeof window.aetherBridge.startLuciaSpecialistTask !== 'function') {
            throw new Error('Run Task needs the Browsphere desktop app.');
          }
          var active = getActiveTab();
          var sourceUrl = active.history[active.historyIndex];
          var taskTabId = createLuciaTaskTab(sourceUrl);
          thread.agentTask.tabId = taskTabId;
          var runId = await window.aetherBridge.startLuciaSpecialistTask(
            text, sourceUrl, getPageData(sourceUrl).title, taskTabId
          );
          thread.agentTask.runId = runId;
          flushLuciaAgentEvents();
          showHudToast('Lucia specialist agents started');
        } catch (err) {
          thread.pending = false;
          var abandonedId = thread.agentTask && thread.agentTask.tabId;
          if (abandonedId) {
            tabs = tabs.filter(function (tab) { return tab.id !== abandonedId; });
            window.aetherBridge?.onCloseTab(abandonedId, 'startup_rollback');
            saveBrowserSession();
          }
          thread.agentTask = null;
          thread.messages.push({role:'assistant',content:'Could not start the task: ' + String(err),error:true});
          renderActiveTab();
        }
      }

      function buildModelChatHistory(thread) {
        // Only confirmed user/assistant exchanges become model context.
        // Failed requests stay visible in the UI but are not passed as invented model replies.
        var valid = [];
        for (var i = 0; i < thread.messages.length; i++) {
          var item = thread.messages[i];
          if (item.role !== 'user') continue;
          var next = thread.messages[i + 1];
          if (next && next.role === 'assistant') {
            if (!next.error) {
              valid.push({ role: 'user', content: item.content });
              valid.push({ role: 'assistant', content: next.content });
            }
            i++;
          } else if (i === thread.messages.length - 1) {
            valid.push({ role: 'user', content: item.content });
          }
        }
        // Keep up to nine past pairs + the current question.
        return valid.slice(-19);
      }

      function scrollMainTabChatToBottom() {
        var messages = document.getElementById('mainChatMessages');
        if (messages) messages.scrollTop = messages.scrollHeight;
      }

      async function submitNewTabChat(overrideText) {
        var input = document.getElementById('newTabChatInput');
        var text = typeof overrideText === 'string' ? overrideText.trim() :
          input ? input.value.trim() : '';
        if (!text) return;
        if (text.length > 8000) {
          showHudToast('Message is too long (8,000 character limit)');
          return;
        }
        if (typeof overrideText !== 'string' &&
            (newTabChatMode === 'browse' ||
              (/^([a-z0-9-]+\.)+[a-z]{2,}(\/.*)?$/i.test(text) && text.indexOf(' ') === -1))) {
          input.value = '';
          navigateToUrl(text);
          return;
        }
        if (luciaIntentPending || getActiveNewTabThread()?.pending) return;
        // The selected model decides whether this message needs real website
        // actions. Ordinary questions stay in the same ChatGPT-like tab.
        if (window.aetherBridge?.isNative &&
            typeof window.aetherBridge.routeLuciaIntent === 'function') {
          luciaIntentPending = true;
          var originalPlaceholder = input?.placeholder || '';
          if (input) {
            input.disabled = true;
            input.placeholder = 'Lucia is deciding how to help…';
          }
          var intent = null;
          var routingFailure = null;
          var originalTabId = getActiveTab()?.id;
          try {
            intent = await window.aetherBridge.routeLuciaIntent(text);
            if (!intent || (intent.mode !== 'chat' && intent.mode !== 'browser_task')) {
              throw new Error('The intent model returned an invalid task classification.');
            }
          } catch (routingError) {
            routingFailure = String(routingError);
          } finally {
            luciaIntentPending = false;
            if (input) {
              input.disabled = false;
              input.placeholder = originalPlaceholder;
            }
          }
          if (getActiveTab()?.id !== originalTabId) return;
          if (routingFailure !== null) {
            // A failed router must never masquerade as a successful chat
            // classification. Show the actual failure in the conversation
            // without calling the text-only chat model or executing a task.
            var failedThread = createLuciaThread(text);
            failedThread.messages.push({role:'user',content:text});
            failedThread.messages.push({
              role:'assistant',
              content:'Lucia could not classify this request. No task was started. Check Settings → AI & ML, then retry. Details: ' + routingFailure,
              error:true
            });
            if (input) input.value = '';
            renderActiveTab();
            renderNewTabHistoryDrawer();
            scrollMainTabChatToBottom();
            return;
          }
          if (intent.mode === 'browser_task') {
            await startNewTabAgentTask(text);
            return;
          }
        }
        var thread = createLuciaThread(text);
        if (thread.pending) return;
        thread.messages.push({ role: 'user', content: text });
        thread.pending = true;
        if (input) input.value = '';
        renderActiveTab();
        renderNewTabHistoryDrawer();
        scrollMainTabChatToBottom();

        try {
          if (!window.aetherBridge || !window.aetherBridge.isNative ||
              typeof window.aetherBridge.askMainTabAi !== 'function') {
            throw new Error('Open the Browsphere Windows app to chat with your configured AI model.');
          }
          var result = await window.aetherBridge.askMainTabAi(buildModelChatHistory(thread), window.browsphereVoice?.enabled === true);
          if (!result || !result.answer || !result.answer.trim()) {
            throw new Error('The AI model returned an empty response.');
          }
          thread.messages.push({
            role: 'assistant', content: result.answer,
            model: result.model || 'Configured AI model'
          });
          window.dispatchEvent(new CustomEvent('browsphere:assistant-text', {
            detail: { text: result.answer, model: result.model || 'Configured AI model' }
          }));
        } catch (err) {
          thread.messages.push({
            role: 'assistant',
            content: 'I could not get an answer from your configured model. ' + String(err) +
              '\n\nCheck Settings → AI & ML, then try again.',
            error: true
          });
        } finally {
          thread.pending = false;
          if (getActiveTab().chatThreadId === thread.id &&
              getActiveTab().history[getActiveTab().historyIndex] === 'browsphere://start') {
            renderActiveTab();
            scrollMainTabChatToBottom();
            var next = document.getElementById('newTabChatInput');
            if (next) next.focus();
          }
          renderNewTabHistoryDrawer();
        }
      }

      function initBrowser() {
        restoreBrowserSession();
        bindAllEvents();
        bindNativeHydrationEvents();
        window.addEventListener('aether:page-loaded', function (event) {
          syncCompletedNativeNavigation(event.detail);
        });
        window.addEventListener('aether:site-permission-denied', function (event) {
          var detail = event.detail || {};
          if (detail.tabId === activeTabId && detail.permission) {
            // Keep decisions explicit. Users may open the lock icon to grant
            // specific site capabilities and reload the webpage.
            var now = Date.now();
            if (!window.__lastSitePermissionToast || now - window.__lastSitePermissionToast > 6000) {
              window.__lastSitePermissionToast = now;
              showHudToast('Blocked ' + detail.permission + '. Use the lock icon to manage site permissions.');
            }
          }
        });
        window.addEventListener('aether:browser-error', function (event) {
          var detail = event.detail || {};
          if (detail.tabId !== activeTabId) return;
          var viewport = document.getElementById('pageViewport');
          if (viewport) {
            viewport.innerHTML = '<div class="max-w-xl mx-auto px-8 py-16 text-center space-y-2">' +
              '<h2 class="text-xl font-semibold">Website could not be opened</h2>' +
              '<p id="browserErrorDetail" class="text-sm opacity-70"></p></div>';
            document.getElementById('browserErrorDetail').textContent =
              String(detail.message || 'An unexpected browser navigation error occurred.');
          }
          showHudToast('Browser navigation failed');
        });
        renderSidebarSection();
        renderActiveTab();
        if (window.aetherBridge && window.aetherBridge.isNative) {
          var recovered = getActiveTab();
          window.aetherBridge.onSwitchTab(recovered.id, recovered.history[recovered.historyIndex]);
        }
        renderDownloadsList();
        applyBrowserSettings();
        setupSelectionListener();
        setupScrollProgress();
        document.addEventListener('visibilitychange', function () {
          if (document.hidden) stopLiveNewsStream();
          else {
            var canvas = document.getElementById('readingCanvas');
            if (canvas && canvas.scrollTop > 80 && document.getElementById('newTabNewsSection')) startLiveNewsStream();
          }
        });
        setupKeyboardShortcuts();
        setupDraggableResizableChat();
        setupQuickPreviewListeners();
        renderNewTabHistoryDrawer();
        setTimeout(function () {
          hydrateHistoryFromNative();
          hydrateDownloadsFromNative();
          hydrateItpFromNative();
          hydrateRouterFromNative();
          hydrateAwsFromNative();
          hydrateCredentialsFromNative();
          hydrateTracesFromNative();
        }, 120);
      }

      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initBrowser);
      } else {
        initBrowser();
      }
    })();
