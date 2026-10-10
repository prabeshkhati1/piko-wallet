// Integration validation against an isolated database. Node 20+; no npm dependencies.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const work=process.env.PIKO_TEST_WORKDIR||path.resolve(root,'../../work/piko-validation');
await mkdir(work,{recursive:true});
const port=process.env.PIKO_TEST_PORT||'33941',origin=`http://127.0.0.1:${port}`;
const binary=process.env.PIKO_BINARY||path.join(root,'target','debug',process.platform==='win32'?'piko-wallet.exe':'piko-wallet');
const database=path.join(work,`integration-${Date.now()}.db`);
const testEnv={...process.env,PIKO_DATABASE:database,PORT:port,SERVER_PORT:port,PIKO_ORIGIN:origin,PIKO_HOST:'127.0.0.1'};delete testEnv.TELEGRAM_BOT_TOKEN;delete testEnv.TELEGRAM_CHAT_ID;delete testEnv.PIKO_SETUP_TOKEN;
const child=spawn(binary,[],{cwd:root,env:testEnv,stdio:['ignore','pipe','pipe']});
let output='';child.stdout.on('data',d=>output+=d);child.stderr.on('data',d=>output+=d);
let cookie='',rev=0,passed=[];
async function request(route,method='GET',body,expect=200,extra={}){
  const r=await fetch(origin+'/api/'+route,{method,headers:{Origin:origin,'Content-Type':'application/json','If-Match':String(rev),...(cookie?{Cookie:cookie}:{}),...extra},body:body===undefined?undefined:JSON.stringify(body)});
  const text=await r.text();assert.equal(r.status,expect,`${route}: ${text}`);
  if(r.headers.get('set-cookie'))cookie=r.headers.get('set-cookie').split(';')[0];
  return text?JSON.parse(text):null;
}
async function state(){const s=await request('state');rev=s.data.revision;return s;}
async function save(kind,item){await request('save/'+kind,'PUT',item);return state();}
function pass(message){passed.push(message);console.log('PASS '+message);}
try{
  for(let i=0;i<100;i++){try{await fetch(origin+'/api/health');break;}catch{await new Promise(r=>setTimeout(r,150));}}
  await request('state','GET',undefined,401);pass('Unauthenticated financial access rejected');
  await request('auth/setup','POST',{password:'piko-test-password-2026'},403,{Origin:'https://untrusted.example'});pass('Untrusted write origins rejected');
  await request('auth/setup','POST',{password:'piko-test-password-2026'});let s=await state();assert.equal(s.data.wallets.length,0);pass('Password setup and authenticated session');
  await request('auth/setup','POST',{password:'piko-test-password-2026'},400);pass('Setup cannot overwrite an existing password');
  s=await save('settings',{...s.data.settings,name:'Demo wallet',currency:'USD',timezone:'Asia/Kathmandu',reminder_hour:9});
  s=await save('wallets',{id:'bank',name:'Bank account',currency:'USD',opening_balance:1200000,color:'#6B4DFF',icon:'loan',include_in_balance:true});
  s=await save('wallets',{id:'cash',name:'Cash',currency:'USD',opening_balance:100000,color:'#14CC9E',icon:'wallet',include_in_balance:true});
  const cats=Object.fromEntries(s.data.categories.map(c=>[c.name,c.id]));
  const transaction=(id,kind,amount,title,date,category_id=null,wallet_id='bank')=>({id,kind,amount,title,date,category_id,wallet_id,note:'',tags:[],to_wallet_id:null,to_amount:null,bill_id:null,bill_due:null});
  for(const t of [transaction('salary','income',420000,'Monthly salary','2026-10-03',cats.Salary),transaction('coffee','expense',850,'Morning coffee','2026-10-10',cats['Food & drinks'],'cash'),transaction('shopping','expense',7600,'A few essentials','2026-10-08',cats.Shopping),transaction('groceries','expense',4800,'Weekly groceries','2026-10-06',cats['Food & drinks']),transaction('transport','expense',1250,'A ride across town','2026-10-09',cats.Transport,'cash'),transaction('dinner','expense',2300,'Dinner with friends','2026-10-07',cats['Food & drinks'])])s=await save('transactions',t);
  const transfer={...transaction('transfer','transfer',25000,'Cash for the week','2026-10-05'),to_wallet_id:'cash',to_amount:25000};
  const before=s.total;s=await save('transactions',transfer);assert.equal(s.total,before);assert.equal(s.total,1703200);pass('Income, expenses and transfers preserve exact balances');
  await request('save/transactions','PUT',transaction('bad','expense',0,'Invalid','2026-10-10'),400);await request('save/transactions','PUT',{...transaction('bad','expense',100,'Missing wallet','2026-10-10'),wallet_id:'missing'},400);pass('Invalid amounts and foreign keys rejected atomically');
  await request('save/wallets','PUT',s.data.wallets[0],409,{'If-Match':'0'});pass('Stale writes rejected');
  const bill=(id,title,amount,next_due,recurrence='month')=>({id,title,amount,next_due,recurrence,wallet_id:'bank',category_id:cats.Home,interval:1,anchor_day:Number(next_due.slice(8)),kind:'expense',paused:false,completed:false,snoozed_until:null,note:''});
  s=await save('bills',bill('rent','Apartment rent',80000,'2026-10-15'));
  s=await save('bills',bill('internet','Internet',4500,'2026-10-09'));
  s=await save('bills',bill('music','Music subscription',999,'2026-11-01'));
  assert.equal(s.total,1703200);pass('Unpaid bills do not reduce actual balances');
  s=await save('budgets',{id:'food-budget',name:'Food & drinks',amount:25000,category_id:cats['Food & drinks']});
  s=await save('budgets',{id:'shopping-budget',name:'Shopping',amount:20000,category_id:cats.Shopping});
  s=await save('loans',{id:'friend',name:'Alex',kind:'lent',amount:50000,currency:'USD',due:'2026-10-20',note:'Trip expenses',records:[]});
  await request('loans/friend/repay','POST',{id:'repayment-1',amount:10000,date:'2026-10-10',note:'First repayment',wallet_id:'bank'});s=await state();assert.equal(s.data.loans[0].records.length,1);assert.equal(s.total,1713200);pass('Loan repayment links to the wallet ledger');
  const baseline=structuredClone(s.data);
  await request('bills/rent/pay','POST',{due:'2026-10-15',date:'2026-10-10'});s=await state();const first=s.total;
  await request('bills/rent/pay','POST',{due:'2026-10-15',date:'2026-10-10'});s=await state();assert.equal(s.total,first);assert.equal(s.data.transactions.filter(t=>t.bill_id==='rent').length,1);assert.equal(s.data.bills.find(b=>b.id==='rent').next_due,'2026-11-15');pass('Repeated settlement does not double-charge and advances recurrence');
  await request('delete/wallets/bank','POST',{},400);pass('Referenced wallets cannot be deleted');
  const backup=await request('export');assert.equal(backup.transactions.length,s.data.transactions.length);await request('import/piko','POST',backup);s=await state();assert.equal(s.total,first);pass('Backup export and restore round-trip');
  await request('import/piko','POST',{...backup,wallets:[]},400);s=await state();assert.equal(s.total,first);pass('Invalid restore leaves current data intact');
  await request('import/piko','POST',baseline);s=await state();await writeFile(path.join(work,'demo-backup.json'),JSON.stringify(s.data,null,2));pass('Reusable demo snapshot saved separately from personal data');
  const manifest=await (await fetch(origin+'/manifest.webmanifest')).json();assert.equal(manifest.name,'Piko Wallet');
  for(const file of ['/icons/icon-192.png','/icons/icon-512.png','/icons/icon-maskable.png','/icons/apple-touch-icon.png','/sw.js'])assert.equal((await fetch(origin+file)).status,200);
  assert.equal((await fetch(origin+'/data/piko.db')).status,404);assert.equal((await fetch(origin+'/api/state',{headers:{Cookie:cookie}})).headers.get('cache-control'),'no-store');pass('PWA assets exist; private storage is not exposed or cacheable');
  await request('auth/logout','POST',{});await request('state','GET',undefined,401);pass('Logout invalidates the server session');
  await request('auth/login','POST',{password:'wrong-password-2026'},401);await request('auth/login','POST',{password:'piko-test-password-2026'});s=await state();assert.equal(s.total,1713200);pass('Login verification and persisted ledger');
  await writeFile(path.join(work,'api-results.json'),JSON.stringify({passed,checks:passed.length,database},null,2));console.log(`${passed.length} integration checks passed`);
}catch(error){console.error(output);throw error;}finally{child.kill();}
