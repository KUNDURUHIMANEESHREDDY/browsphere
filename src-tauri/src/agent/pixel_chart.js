(() => {
  const forbidden='form,input,textarea,select,[contenteditable],[hidden],[inert],[aria-hidden="true"],nav,aside,header,footer,dialog,[role="dialog"]';
  if(document.querySelector('dialog[open],[aria-modal="true"]'))return null;
  const visible=e=>{
    if(!e||e.closest(forbidden))return false;
    for(let n=e;n&&n.nodeType===1;n=n.parentElement){const s=getComputedStyle(n);if(s.display==='none'||s.visibility!=='visible'||Number(s.opacity)===0)return false;}
    const b=e.getBoundingClientRect();return b.width>0&&b.height>0;
  };
  const regions=[];
  let omitted=0;
  const candidates=Array.from(document.querySelectorAll('figure canvas,figure img,canvas[role="img"],img[role="img"]')).filter(visible);
  for(const el of candidates.slice(0,16)){
    const figure=el.closest('figure'),cap=figure?.querySelector('figcaption');
    const caption=visible(cap)?(cap.innerText||'').replace(/\s+/g,' ').trim():'';
    const b=el.getBoundingClientRect();
    // Require a visible chart caption and the entire graphic in the viewport.
    // Reject covered regions, interactive/editor surfaces and huge captures.
    if(regions.length>=2||!caption||caption.length>160||!/\b(chart|graph|plot)\b/i.test(caption)||
      b.width<160||b.height<100||b.width>1200||b.height>900||b.left<0||b.top<0||b.right>innerWidth||b.bottom>innerHeight||
      (el.tagName==='IMG'&&(!el.complete||el.naturalWidth===0))||
      ![.05,.5,.95].every(x=>[.05,.5,.95].every(y=>{const hit=document.elementFromPoint(b.left+b.width*x,b.top+b.height*y);return hit===el;}))){omitted++;continue;}
    regions.push({kind:el.tagName.toLowerCase(),caption,x:b.left+scrollX,y:b.top+scrollY,width:b.width,height:b.height});
  }
  omitted+=Math.max(0,candidates.length-16);
  return {url:location.href,title:document.title,viewport:{width:innerWidth,height:innerHeight,scroll_x:scrollX,scroll_y:scrollY,scale:devicePixelRatio,origin:location.href},regions,omitted};
})()