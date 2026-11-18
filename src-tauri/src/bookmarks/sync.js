/* This extension reads bookmarks only; its native host can only sync bookmarks. */
const api=browser.bookmarks, storage=browser.storage.local;
let port, waiter, busy=false;
function rpc(changes,epoch){return new Promise((resolve,reject)=>{
 if(!port){port=browser.runtime.connectNative('app.starpersona.bookmarks');port.onDisconnect.addListener(()=>{port=null;waiter?.reject(new Error('星相未连接'));waiter=null});port.onMessage.addListener(r=>{if(r.error)waiter?.reject(new Error(r.error));else waiter?.resolve(r.result);waiter=null})}
 waiter={resolve,reject};port.postMessage({changes,epoch});
})}
function value(b){if(!b?.url||b.url.length>4096)return null;try{const u=new URL(b.url);if(!['https:','http:'].includes(u.protocol)||u.username||u.password)return null}catch{return null}return {title:[...(b.title||'')].slice(0,500).join(''),url:b.url}}
function flat(nodes,out={}){for(const n of nodes){out[n.id]=n;if(n.children)flat(n.children,out)}return out}
const equal=(a,b)=>a?.url===b?.url && a?.title===b?.title;
async function tick(){if(busy)return;busy=true;try{
 const nodes=flat(await api.getTree());let {state}=await storage.get('state');
 if(!state){state={ignore:Object.keys(nodes),links:{},folder:null};await storage.set({state})} // Never upload existing private bookmarks on first run.
 const links=state.links;
 // Ask before computing changes on first use. A disabled/re-enabled library starts a new consent period.
 if(state.epoch===undefined){const info=await rpc([],null);if(!info.enabled)return;state.epoch=info.epoch;state.ignore=Object.keys(nodes).filter(id=>!links[id]);await storage.set({state});}
 const ignored=new Set(state.ignore);
 if(!nodes[state.folder])state.folder=(await api.create({parentId:'toolbar_____',title:'星相共享书签'})).id;
 const changes=[];
 for(const [id,n] of Object.entries(nodes)){if(!value(n)||ignored.has(id)||links[id])continue;links[id]={id:crypto.randomUUID(),base:null};}
 for(const [id,link] of Object.entries(links)){const after=value(nodes[id]);if(!equal(link.base,after)){if(changes.length>=32)break;changes.push({id:link.id,before:link.base,after})}}
 await storage.set({state});const remote=await rpc(changes,state.epoch);
 if(!remote.enabled || remote.reset){for(const [id,link] of Object.entries(links))if(!link.base)delete links[id];delete state.epoch;await storage.set({state});return;}
 const sent=new Set(changes.map(c=>c.id));const mapped=new Set();
 for(const [local,link] of Object.entries(links)){mapped.add(link.id);const old=value(nodes[local]);if(!equal(old,link.base)&&!sent.has(link.id))continue;const next=remote.items[link.id];
 if(!next){if(nodes[local]&&old)await api.remove(local);delete links[local];continue}
 if(!old){delete links[local];mapped.delete(link.id);continue}if(!equal(old,next))await api.update(local,next);link.base=next;}
 for(const [id,b] of Object.entries(remote.items)){if(mapped.has(id))continue;const n=await api.create({parentId:state.folder,...b});links[n.id]={id,base:b};await storage.set({state})}
 await storage.set({state});
 }catch(e){console.warn('StarPersona bookmarks:',String(e))}finally{busy=false}}
setInterval(tick,3000);tick();
