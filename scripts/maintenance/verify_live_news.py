from pathlib import Path
import re,subprocess
p=Path(r"C:\Users\himan\Downloads\Browsphere\ui\public\index.html")
s=p.read_text(encoding="utf8")
blocks=re.findall(r'<script(?:\s[^>]*)?>([\s\S]*?)</script>',s)
files=[]
for i,b in enumerate(blocks):
 if b.strip():
  q=p.parent/f'_syntax_check_{i}.js'
  q.write_text(b,encoding='utf8')
  files.append(q)
for f in files:
 try:
  r=subprocess.run(['node','--check',str(f)],capture_output=True,text=True)
  print(f.name, 'PASS' if r.returncode==0 else 'FAIL', r.stderr[:1200])
 except FileNotFoundError:
  print('Node not installed')
 f.unlink()
print('fixture gone:', 'Sub-Threshold Surface Code Scaling' not in s)
print('Hacker News API:', 'hacker-news.firebaseio.com/v0/' in s)
print('news section:', 'newTabNewsSection' in s)
