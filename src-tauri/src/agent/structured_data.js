(() => {
  "use strict";
  const excluded = 'form,input,textarea,select,button,[contenteditable],[hidden],[inert],[aria-hidden="true"],nav,aside,header,footer,dialog,[role="dialog"],[role="navigation"]';
  const visible = el => {
    if (!el || el.closest(excluded)) return false;
    for (let n=el;n && n.nodeType===1;n=n.parentElement) {
      const s=getComputedStyle(n);
      if (s.display==='none'||s.visibility!=='visible'||Number(s.opacity)===0) return false;
    }
    const b=el.getBoundingClientRect();
    return b.width>0&&b.height>0;
  };
  const text = el => {
    if (!visible(el)) return null;
    const walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT);
    const parts=[];
    for(let n=walker.nextNode();n;n=walker.nextNode()) {
      if(visible(n.parentElement)) parts.push(n.nodeValue);
      if(parts.length>128) return null;
    }
    const value=parts.join(' ').replace(/\s+/g,' ').trim();
    return value.length<=160?value:null;
  };
  const tables=[],charts=[];
  let omitted_tables=0,omitted_charts=0;
  const roots=Array.from(document.querySelectorAll('main,[role="main"]')).filter(visible);
  const root=roots.length===1?roots[0]:document.body;
  if(!root || document.querySelector('dialog[open],[aria-modal="true"]')) return null;
  const candidates=Array.from(root.querySelectorAll('table')).filter(visible);
  for (const table of candidates.slice(0,12)) {
    if(tables.length>=4){omitted_tables++;continue;}
    const rows=Array.from(table.rows).filter(r=>r.closest('table')===table&&visible(r));
    const caption=table.caption?text(table.caption):'';
    const head=rows[0];
    if(!head||rows.length<2||rows.length>21||caption===null||table.querySelector('table,input,textarea,select,[contenteditable]')){omitted_tables++;continue;}
    const matrix=[];
    let invalid=false;
    for(const row of rows){
      const cells=Array.from(row.cells);
      if(cells.length<2||cells.length>10||cells.some(c=>!visible(c)||c.colSpan!==1||c.rowSpan!==1||c.closest('table')!==table)){invalid=true;break;}
      const values=cells.map(text);
      if(values.some(v=>v===null)||values.length!==(matrix[0]?.length||values.length)){invalid=true;break;}
      matrix.push(values);
    }
    if(invalid||!Array.from(head.cells).every(c=>c.tagName==='TH')||matrix[0].some(v=>!v)||new Set(matrix[0]).size!==matrix[0].length){omitted_tables++;continue;}
    tables.push({caption,headers:matrix[0],rows:matrix.slice(1)});
  }
  omitted_tables+=Math.max(0,candidates.length-12);
  // Accept only SVG charts with a visible caption and explicitly paired
  // rendered label/value text in individual mark groups. Never infer values
  // from geometry, aria strings, script state, metadata or image pixels.
  const graphics=Array.from(root.querySelectorAll('figure svg,svg[role="img"],canvas,figure img')).filter(visible);
  const number=/^[+-]?(?:\d{1,12}(?:\.\d{1,6})?|\.\d{1,6})(?:\s?%|\s?[A-Za-z]{1,8})?$/;
  for(const graphic of graphics.slice(0,20)){
    if(charts.length>=4||graphic.tagName.toLowerCase()!=='svg'){omitted_charts++;continue;}
    const figure=graphic.closest('figure');
    const caption=figure?text(figure.querySelector('figcaption')):null;
    if(!caption||!/chart|graph|plot/i.test(caption)){omitted_charts++;continue;}
    const groups=Array.from(graphic.querySelectorAll('g')).filter(g=>visible(g)&&!g.querySelector('g')&&g.querySelector('rect,circle,path,line,polygon,ellipse'));
    const points=[];
    let invalid=false;
    for(const group of groups.slice(0,31)){
      const labels=Array.from(group.querySelectorAll('text')).filter(visible).map(text);
      if(labels.length!==2||labels.some(v=>!v)||number.test(labels[0])||!number.test(labels[1])){invalid=true;break;}
      points.push({label:labels[0],value:labels[1]});
    }
    if(invalid||groups.length<2||groups.length>30||new Set(points.map(p=>p.label)).size!==points.length){omitted_charts++;continue;}
    // Other visible text can include axes/legends; do not claim to have
    // interpreted them, series mapping, scales or unlabeled marks.
    charts.push({caption,points});
  }
  omitted_charts+=Math.max(0,graphics.length-20);
  return {url:location.href,title:document.title,tables,charts,omitted_tables,omitted_charts};
})()