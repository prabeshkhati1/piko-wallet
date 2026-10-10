// Always uses a separate, disposable database. Never tests against the user's ledger.
const {spawn,spawnSync}=require('node:child_process');
const fs=require('node:fs');const path=require('node:path');
const root=path.resolve(__dirname,'..');
const work=process.env.PIKO_TEST_WORKDIR||path.resolve(root,'../../work/piko-validation');fs.mkdirSync(work,{recursive:true});
const binary=process.env.PIKO_BINARY||path.join(root,'target','debug',process.platform==='win32'?'piko-wallet.exe':'piko-wallet');
const setup=spawnSync(process.execPath,[path.join(__dirname,'api.mjs')],{cwd:root,env:{...process.env,PIKO_BINARY:binary,PIKO_TEST_WORKDIR:work},stdio:'inherit'});if(setup.status!==0)process.exit(setup.status||1);
const origin='http://127.0.0.1:33942';const env={...process.env,PIKO_BINARY:binary,PIKO_TEST_WORKDIR:work,PIKO_TEST_URL:origin,PIKO_DATABASE:path.join(work,`browser-${Date.now()}.db`),PORT:'33942',SERVER_PORT:'33942',PIKO_HOST:'127.0.0.1',PIKO_ORIGIN:origin};
delete env.TELEGRAM_BOT_TOKEN;delete env.TELEGRAM_CHAT_ID;delete env.PIKO_SETUP_TOKEN;
const server=spawn(binary,[],{cwd:root,env,stdio:['ignore','pipe','pipe']});server.stderr.on('data',d=>process.stderr.write(d));
(async()=>{try{for(let i=0;i<100;i++){try{await fetch(origin+'/api/health');break;}catch{await new Promise(r=>setTimeout(r,150));}}
for(const script of ['browser.cjs','webkit.cjs'])await new Promise((resolve,reject)=>{const child=spawn(process.execPath,[path.join(__dirname,script)],{cwd:root,env,stdio:'inherit'});child.on('exit',code=>code===0?resolve():reject(new Error(`${script} exited ${code}`)));});
}finally{server.kill();}})().catch(e=>{console.error(e);process.exitCode=1;});
