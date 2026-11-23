const fs = require("fs");
const vm = require("vm");
const source = fs.readFileSync("ui/public/src/ai-preferences.js","utf8");
const ctx = {window:{}};
vm.runInNewContext(source, ctx);
const config = {
  profile:"default",region:"ap-southeast-2",bedrock_model:"amazon.nova-lite-v1:0",bedrock_enabled:false,
  s3_bucket:"browsphere-private-test",s3_key:"browsphere/settings-backup.json",dynamodb_table:"BrowsphereSync",
  sync_namespace:"private",cognito_client_id:"exampleclient",lambda_function:"BrowsphereWorker"
};
const body = ctx.window.renderMacAiPreferences({config:{ollama_model:"qwen3.5:4b",litellm_model:"gpt-4o-mini"},aws:config,awsConnecting:false,awsStatus:"Connected",
awsServices:[{service:"S3",status:"ready",detail:"Private bucket ready"},{service:"Bedrock",status:"available",detail:"Model listed"}],
awsAccountHint:"AWS account ending 1234"});
for(const expected of ["data-action=\"aws-connect\"","Connect AWS","Advanced AWS service settings","Private bucket ready","cfgAwsProfile","cfgAwsStatus","cfgAwsBedrockEnabled"]){
  if(!body.includes(expected)) { throw Error("Missing element "+expected); }
}
const pending = ctx.window.renderMacAiPreferences({config:{},aws:config,awsConnecting:true});
if(!pending.includes("Connecting…") || !pending.includes("disabled")) throw Error("Pending button not disabled");
if(body.indexOf("Connect AWS") > body.indexOf("Default Intelligence")) throw Error("AWS connect not prominent");
console.log("PASS macOS AWS Connect component: 9 assertions");
