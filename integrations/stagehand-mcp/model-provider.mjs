// Thin OpenAI-style message -> Ollama adapter following Stagehand's
// documented bring-your-own-LLM callback. Stagehand owns observe/extract.
export function createOllamaModel({model = 'qwen2.5:3b-instruct', request = fetch} = {}) {
  if (!/^[a-zA-Z0-9._:/-]{1,90}$/.test(model)) throw Error('Invalid locally installed model name');
  return {
    async generate(params) {
      if (params?.responseFormat?.type !== 'json_schema' ||
          !params.responseFormat.schema || typeof params.responseFormat.schema !== 'object') {
        throw Error('Stagehand requires structured, schema-constrained response');
      }
      const messages = [];
      if (params.systemPrompt) messages.push({role:'system',content:params.systemPrompt});
      for (const item of params.messages || []) {
        const blocks=Array.isArray(item.content)?item.content:[item.content];
        if (!blocks.every(b=>b?.type==='text')) throw Error('Images and file content not forwarded to a model');
        messages.push({role:item.role,content:blocks.map(b=>b.text).join('\n')});
      }
      const response = await request('http://127.0.0.1:11434/api/chat',{
        method:'POST',headers:{'Content-Type':'application/json'},
        signal:AbortSignal.timeout(90000),
        body:JSON.stringify({
          model,stream:false,format:params.responseFormat.schema,messages,
          options:{temperature:0,num_ctx:3072,num_predict:256}
        })
      });
      if (!response.ok) throw Error('Local model unavailable: '+response.status);
      const data=await response.json();
      const output=data?.message?.content;
      if (typeof output !== 'string' || output.length>65536) throw Error('Invalid model output');
      const structured=JSON.parse(output);
      return {role:'assistant',content:{type:'text',text:output},
        outputFormat:'json_schema',structuredContent:structured};
    }
  };
}
