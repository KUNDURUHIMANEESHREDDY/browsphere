// Real upstream Stagehand request policy regression. A public-looking fixture
// must not navigate into a different local host through an HTTP redirect.
// Uses the official Stagehand v4 domain policy, not a homegrown CDP interceptor.
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
import {stagehandRead} from './stagehand-readonly.mjs';

let protectedRequests=0;
const protectedServer=createServer((req,res)=>{
  protectedRequests++;
  res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});
  res.end('<title>Protected local service</title>');
});
let redirectTarget='';
const sourceServer=createServer((req,res)=>{
  if (req.url==='/redirect') {
    res.writeHead(302,{Location:redirectTarget});res.end();return;
  }
  res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});
  res.end('<title>Research fixture</title>');
});
async function listen(server,host) {
  await new Promise((resolve,reject)=>{
    server.once('error',reject);
    server.listen(0,host,resolve);
  });
}
async function close(server) {
  if (server.listening) await new Promise(resolve=>server.close(resolve));
}
try {
  await listen(protectedServer,'localhost');
  redirectTarget=`http://localhost:${protectedServer.address().port}/private`;
  // Prove the forbidden target is reachable *without* the domain policy, so
  // a rejected navigation does not falsely pass due to DNS/network failure.
  const probe=await fetch(redirectTarget);
  assert.equal(probe.status,200,'Local disallowed target is not reachable');
  protectedRequests=0;
  await listen(sourceServer,'127.0.0.1');
  const sourcePort=sourceServer.address().port;
  const requested=`http://127.0.0.1:${sourcePort}/redirect`;
  await assert.rejects(
    stagehandRead({
      url:requested,
      instruction:'Read the title of the current source',
      mode:'inspect'
    },{ci:true,fixturePort:String(sourcePort)}),
    /./,
    'An upstream Stagehand research redirect must not return data'
  );
  assert.equal(protectedRequests,0,
    'The forbidden localhost service was reached before post-navigation verification');
  console.log('STAGEHAND_DOMAIN_POLICY_REDIRECT_BLOCKED '+JSON.stringify({
    upstream:'@browserbasehq/stagehand',
    nativeDomainPolicy:true,
    forbiddenRequestCount:protectedRequests,
    isolatedFixture:true,
    realModelTask:false
  }));
} finally {
  await close(sourceServer);
  await close(protectedServer);
}
