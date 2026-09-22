// Development-only importer. Run with NODE_PATH pointing to Playwright's installation.
// Input: an unpacked Baseline checkout at the pinned revision. No runtime CSS dependency.
const fs = require('fs'), path = require('path');
const { chromium } = require('playwright');
const source = process.argv[2];
const root = path.resolve(__dirname, '..');
const revision = '8c56e831e1abb1d3841c4ffdecbe06b5182fbc68';
const keys = ['background-primary','background-primary-alt','background-secondary','background-secondary-alt','text-normal','text-muted','text-faint','background-modifier-border','background-modifier-border-hover','background-modifier-border-focus','background-modifier-hover','interactive-normal','interactive-hover','interactive-accent','interactive-accent-hover','text-on-accent','text-selection','color-red','color-green','color-blue','color-yellow','color-purple','color-cyan','color-base-00','color-base-05','color-base-10'];
// Obsidian's neutral semantic defaults, needed by partial Baseline schemes.
const base = `
body { --accent-h: 230; --accent-s: 80%; --accent-l: 65%; --color-accent-hsl:var(--accent-h),var(--accent-s),var(--accent-l); --color-accent:hsl(var(--color-accent-hsl)); --interactive-accent:var(--color-accent); --interactive-accent-hover:var(--color-accent); --text-on-accent:white; --background-primary:var(--color-base-00); --background-primary-alt:var(--color-base-10); --background-secondary:var(--color-base-20); --background-secondary-alt:var(--color-base-30); --text-normal:var(--color-base-100); --text-muted:var(--color-base-70); --text-faint:var(--color-base-50); --background-modifier-border:var(--color-base-30); --background-modifier-border-hover:var(--color-base-35); --background-modifier-border-focus:var(--color-base-40); --interactive-normal:var(--color-base-00); --interactive-hover:var(--color-base-10); --background-modifier-hover:rgba(var(--mono-rgb-100),.067); --text-selection:hsla(var(--color-accent-hsl),.2); }
.theme-light { --mono-rgb-100:0,0,0; --color-base-00:#ffffff; --color-base-05:#fcfcfc; --color-base-10:#fafafa; --color-base-20:#f6f6f6; --color-base-25:#e3e3e3; --color-base-30:#e0e0e0; --color-base-35:#d4d4d4; --color-base-40:#bdbdbd; --color-base-50:#ababab; --color-base-60:#707070; --color-base-70:#5c5c5c; --color-base-100:#222222; }
.theme-dark { --mono-rgb-100:255,255,255; --color-base-00:#1e1e1e; --color-base-05:#212121; --color-base-10:#242424; --color-base-20:#262626; --color-base-25:#2a2a2a; --color-base-30:#363636; --color-base-35:#3f3f3f; --color-base-40:#555555; --color-base-50:#666666; --color-base-60:#999999; --color-base-70:#b3b3b3; --color-base-100:#dadada; --interactive-normal:var(--color-base-30); --interactive-hover:var(--color-base-35); }
`;
(async () => {
 const browser = await chromium.launch({channel:'chrome',headless:true});
 const page = await browser.newPage();
 await page.setContent('<html><head></head><body><div class="mod-sidedock"><span id="chrome"></span></div><span id="content"></span></body></html>');
 await page.addStyleTag({content:base});
 await page.addStyleTag({content:fs.readFileSync(path.join(source,'theme.css'),'utf8')});
 const files = fs.readdirSync(path.join(source,'src/color-schemes')).filter(x=>x.endsWith('.scss') && x!=='contrast.scss');
 const names = { 'rose-pine':'Rosé Pine','catppuccin':'Catppuccin','frappe':'Frappé' };
 const schemes = [];
 let credits = `# Baseline palette sources\n\nPinned revision: ${revision}\nhttps://github.com/aaaaalexis/obsidian-baseline\n\n`;
 for(const file of files) {
  const raw=fs.readFileSync(path.join(source,'src/color-schemes',file),'utf8');
  const id=file.replace('.scss','');
  schemes.push({id,name:names[id]||id[0].toUpperCase()+id.slice(1),modes:['light','dark'].filter(m=>raw.includes(`.theme-${m}.${id}-${m}`))});
  credits+=`## ${id}\n\n${(raw.match(/\/\*([\s\S]*?)\*\//)||[])[1]?.trim()||'Baseline'}\n\n`;
 }
 const palettes=[];
 for(const scheme of schemes) for(const mode of scheme.modes) for(const contrast of ['normal','tonal',mode==='light'?'vivid':'black']) for(const scheme_accent of [false,true]) {
  const tokens=await page.evaluate(({scheme,mode,contrast,scheme_accent,keys})=>{
   document.body.className=`theme-${mode} ${scheme.id}-${mode} is-focused accented-interface ${scheme_accent?'color-scheme-accent':''} contrast-${mode}${contrast==='normal'?'':'-'+contrast} layout-frame`;
   const canvas=document.createElement('canvas'); canvas.width=canvas.height=1;
   const ctx=canvas.getContext('2d',{willReadFrequently:true});
   const resolve=(el,key)=>{
    el.style.color=`var(--${key})`;
    const color=getComputedStyle(el).color;
    ctx.clearRect(0,0,1,1);ctx.fillStyle=color;ctx.fillRect(0,0,1,1);
    return '#'+Array.from(ctx.getImageData(0,0,1,1).data).map(v=>v.toString(16).padStart(2,'0')).join('');
   };
   return Object.fromEntries(['content','chrome'].map(area=>[area,Object.fromEntries(keys.map(k=>[k,resolve(document.getElementById(area),k)]))]));
  },{scheme,mode,contrast,scheme_accent,keys});
  palettes.push({id:scheme.id,mode,contrast,scheme_accent,...tokens});
 }
 fs.writeFileSync(path.join(root,'src/themes/baseline.json'),JSON.stringify({revision,schemes,palettes},null,2)+'\n');
 fs.mkdirSync(path.join(root,'licenses/baseline'),{recursive:true});
 fs.copyFileSync(path.join(source,'LICENSE.txt'),path.join(root,'licenses/baseline/LICENSE.txt'));
 fs.writeFileSync(path.join(root,'licenses/baseline/SOURCES.md'),credits);
 await browser.close();
 console.log(`${schemes.length} schemes, ${palettes.length} resolved palettes`);
})().catch(e=>{console.error(e);process.exit(1)});
