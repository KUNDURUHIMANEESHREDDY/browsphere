'use strict';
// Deterministic test-fixture decision helper, NOT a production browser bypass.
// Only chooses the currently observed close control when the native page shows
// that the requested details dialog really is open.
function closeObservedDetailsDialog(page, nodes) {
  if (typeof page!=='string' || !Array.isArray(nodes)) return null;
  const open=/(?:active dialog:|details dialog is open|dialog is open)/i.test(page) &&
    !/(?:dialog not opened|dialog closed successfully)/i.test(page);
  if (!open) return null;
  const close=nodes.find(node=>node && node.label==='Close details' &&
    node.tag==='button' && typeof node.node_id==='string' && !node.is_sensitive);
  return close ? {type:'click',node_id:close.node_id,
    element_label:'Close details',is_submit:false} : null;
}
module.exports={closeObservedDetailsDialog};
