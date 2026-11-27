'use strict';
// Deterministic test-fixture decision helper, NOT a production browser bypass.
// Only chooses the currently observed close control when the native page shows
// that the requested details dialog really is open.
function closeObservedDetailsDialog(page, nodes) {
  if (typeof page!=='string' || !Array.isArray(nodes)) return null;
  // Other background controls (e.g. "Keyboard dialog not opened") are
  // irrelevant; the active native modal and its live close button decide.
  const open=/(?:active dialog:|details dialog is open|dialog is open)/i.test(page);
  if (!open) return null;
  const close=nodes.find(node=>node && node.label==='Close details' &&
    node.tag==='button' && typeof node.node_id==='string' && !node.is_sensitive);
  return close ? {type:'click',node_id:close.node_id,
    element_label:'Close details',is_submit:false} : null;
}
// Keyboard-only modal has no close button; Escape is permitted ONLY when
// the active native DOM exposes its labeled dialog target.
function escapeObservedKeyboardDialog(page,nodes) {
  if (typeof page!=='string'||!Array.isArray(nodes)) return null;
  const modal=nodes.find(n=>n&&n.label==='Keyboard dialog' &&
    n.role==='dialog' && typeof n.node_id==='string' && !n.is_sensitive);
  if (!modal || !/(?:active dialog:|keyboard dialog is open|press escape to close this dialog)/i.test(page))
    return null;
  return {type:'press_key',node_id:modal.node_id,key:'Escape'};
}
module.exports={closeObservedDetailsDialog,escapeObservedKeyboardDialog};
