"""Opt-in integration fixture. No user profile is opened; a pre-existing native host is never replaced."""
import json, os, pathlib, sys, time, subprocess, threading, http.server, uuid
root=pathlib.Path(sys.argv[1]); repo=pathlib.Path(__file__).resolve().parents[1]
host=repo/'src-tauri/target/release/starpersona'; assert host.is_file()
manifest=pathlib.Path.home()/'Library/Application Support/Mozilla/NativeMessagingHosts/app.starpersona.bookmarks.json'; manifest.parent.mkdir(parents=True,exist_ok=True)
requests={}; replies={}; cv=threading.Condition(); processes=[]; uid=str(uuid.uuid4()); url='https://example.org/'+uid
class H(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def send(self,obj):
  data=json.dumps(obj).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 def do_GET(self):
  with cv:self.send(requests.pop(self.path,None))
 def do_POST(self):
  with cv:replies[self.path]=json.loads(self.rfile.read(int(self.headers['Content-Length'])));cv.notify_all()
  self.send(True)
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),H);threading.Thread(target=server.serve_forever,daemon=True).start();origin='http://127.0.0.1:'+str(server.server_port)
def command(which,action,arg=None):
 path='/'+which
 with cv:
  replies.pop(path,None);requests[path]=[action,arg,url];assert cv.wait_for(lambda:path in replies,timeout=16),'no probe reply '+which;return replies.pop(path)
def wait(which,title):
 for _ in range(26):
  rows=command(which,'find');
  if ([r['title'] for r in rows]==([title] if title else [])):return rows
  time.sleep(.55)
 raise AssertionError((which,title,rows))
def launch(which):
 d=root/which;d.mkdir(exist_ok=True);profile=d/'profile';profile.mkdir(exist_ok=True);d0=repo/'src-tauri/src/bookmarks'; mf=json.loads((d0/'manifest.json').read_text());mf['background']['scripts'].append('qa.js');mf['permissions'].append('http://127.0.0.1/*');(d/'manifest.json').write_text(json.dumps(mf));(d/'sync.js').write_bytes((d0/'sync.js').read_bytes());(d/'qa.js').write_text('''const testUrl=ENDPOINT;let pending=false;setInterval(async()=>{if(pending)return;pending=true;try{const job=await(await fetch(testUrl)).json();if(!job)return;let [act,arg,url]=job;let result;if(act==='create')result=await browser.bookmarks.create({title:arg,url});else result=await browser.bookmarks.search({url});await fetch(testUrl,{method:'POST',body:JSON.stringify(result)})}finally{pending=false}},250);'''.replace('ENDPOINT',json.dumps(origin+'/'+which)));log=(d/'log').open('ab');p=subprocess.Popen(['/Applications/Camoufox.app/Contents/MacOS/camoufox','--headless','-no-remote','-profile',str(profile),'about:blank'],env={**os.environ,'STARPERSONA_BOOKMARK_SOCKET':str(root/'s'),'CAMOU_CONFIG_1':json.dumps({'addons':[str(d)]})},stdout=log,stderr=log);processes.append(p);return p
def native(changes,epoch):
 import socket
 with socket.socket(socket.AF_UNIX) as sock:
  sock.connect(str(root/'s'));sock.sendall((json.dumps({'method':'bookmark.sync','params':{'changes':changes,'epoch':epoch}})+'\n').encode());return json.loads(sock.makefile().readline())['result']
def external(changes):
 return native(changes,native([],None)['epoch'])['items']
with manifest.open('x') as f: json.dump({'name':'app.starpersona.bookmarks','description':'temporary StarPersona QA host','path':str(host),'type':'stdio','allowed_extensions':['bookmarks@starpersona.local']},f)
try:
 launch('A');launch('B');command('A','find');command('B','find');time.sleep(4);command('A','create','A created');wait('B','A created');lib=external([]);key=next(k for k,v in lib.items() if v['url']==url);older=lib[key];edit={'title':'edited centrally','url':url};external([{'id':key,'before':older,'after':edit}]);wait('A','edited centrally');wait('B','edited centrally');external([{'id':key,'before':edit,'after':None}]);wait('A',None);wait('B',None);print('PASS real Firefox: A add -> B, central edit and delete -> A+B')
except Exception:
 for d in ('A','B'):
  if (root/d/'log').exists():print(d,(root/d/'log').read_text(errors='replace')[-1900:])
 raise
finally:
 for p in processes:
  if p.poll() is None:p.terminate()
 for p in processes:
  try:p.wait(timeout=8)
  except subprocess.TimeoutExpired:p.kill();p.wait()
 manifest.unlink();server.server_close()
