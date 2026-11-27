'use strict';
// A model tag may contain '/', ':', dots or a provider namespace.
// Only whitespace-delimited orchestrator stage separators may split routes.
function traceRouteMatchesModel(route, provider, model) {
  if (!route || !provider || !model) return false;
  const expected=(String(provider)+':'+String(model)).toLowerCase();
  return String(route).toLowerCase().split(/\s+\/\s+/)
    .some(part=>part===expected || part.startsWith(expected+' '));
}
module.exports={traceRouteMatchesModel};
