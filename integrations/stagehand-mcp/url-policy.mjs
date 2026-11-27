// Only explicitly allowlisted public research sources are reachable through
// Stagehand. No browsing into local services, cloud metadata or private apps.
import { isIP } from 'node:net';

const BASE_ALLOWED = new Set(['developer.mozilla.org','docs.python.org','www.w3.org','web.dev']);
export function allowedResearchUrl(raw, { ci = false, fixturePort = '' } = {}) {
  if (typeof raw !== 'string' || raw.length > 2048) throw Error('Invalid research URL');
  let u;
  try { u = new URL(raw); } catch { throw Error('Invalid research URL'); }
  if (u.username || u.password || u.hash || !['https:','http:'].includes(u.protocol)) throw Error('Rejected research URL');
  if (u.hostname === '127.0.0.1' && ci && /^[0-9]{4,5}$/.test(String(fixturePort)) &&
      Number(u.port) === Number(fixturePort) && u.protocol === 'http:') {
    return u.toString();
  }
  if (u.protocol !== 'https:' || u.port || isIP(u.hostname) ||
      !BASE_ALLOWED.has(u.hostname.toLowerCase())) {
    throw Error('Source not on the explicit read-only public research allowlist');
  }
  return u.toString();
}
