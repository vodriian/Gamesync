// All profiles are invented fixtures. The prototype never reads a personal library.
const games = [
  {id:1145360,title:'Hades',style:'Action',activities:['Never-ending'],min:20,ideal:35,setup:2,mechanical:2,cognitive:1,narrative:1,onboarding:1,stop:'Between rooms',status:'playing',favorite:true,installed:['pc','deck'],deck:true,played:18,last:2,note:'A familiar run, with little setup and a clear place to stop.'},
  {id:413150,title:'Stardew Valley',style:'Life sim',activities:['Explore'],min:15,ideal:30,setup:2,mechanical:1,cognitive:2,narrative:1,onboarding:1,stop:'At the end of a day',status:'playing',favorite:true,installed:['pc','deck'],deck:true,played:42,last:5,note:'A day on the farm gives this session a small, satisfying finish.'},
  {id:367520,title:'Hollow Knight',style:'Adventure',activities:['Explore','Weird'],min:25,ideal:60,setup:2,mechanical:3,cognitive:2,narrative:1,onboarding:1,stop:'At a bench',status:'backlog',favorite:false,installed:['pc'],deck:true,played:4,last:120,note:'An old corner of your library to explore, when you have energy to spare.'},
  {id:620,title:'Portal 2',style:'Puzzle',activities:['Weird','Multiplayer'],min:15,ideal:30,setup:2,mechanical:2,cognitive:3,narrative:1,onboarding:1,stop:'Between chambers',status:'want',favorite:false,installed:['pc','deck'],deck:true,played:3,last:80,note:'A few self-contained chambers, if you feel like working out a puzzle.'},
  {id:1091500,title:'Cyberpunk 2077',style:'RPG',activities:['Shoot','Drive','Explore'],min:45,ideal:90,setup:4,mechanical:2,cognitive:3,narrative:3,onboarding:1,stop:'Between missions',status:'playing',favorite:true,installed:['pc'],deck:true,played:22,last:7,note:'Pick up your story with enough room for a mission and a little wandering.'},
  {id:990080,title:'Hogwarts Legacy',hidden:true,style:'RPG',activities:['Fly','Explore'],min:30,ideal:60,setup:4,mechanical:2,cognitive:2,narrative:2,onboarding:3,stop:'Between quests',status:'backlog',favorite:false,installed:[],deck:true,played:0,last:500,note:'An unplayed world to settle into. Leave room for the opening tutorial.'},
  {id:646570,title:'Slay the Spire',style:'Cards',activities:['Play cards','Never-ending'],min:10,ideal:45,setup:1,mechanical:1,cognitive:3,narrative:1,onboarding:1,stop:'Save between encounters',status:'backlog',favorite:false,installed:['pc','deck'],deck:true,played:7,last:210,note:'A forgotten favorite style, with time to think and room to save mid-run.'},
  {id:753640,title:'Outer Wilds',style:'Exploration',activities:['Fly','Explore','Weird'],min:22,ideal:50,setup:3,mechanical:2,cognitive:3,narrative:3,onboarding:2,stop:'At the end of a loop',status:'backlog',favorite:false,installed:['pc'],deck:null,played:1,last:360,note:'Return to a mystery you left behind, with enough attention to follow a clue.'},
  {id:2379780,title:'Balatro',style:'Cards',activities:['Play cards','Never-ending','Weird'],min:10,ideal:30,setup:1,mechanical:1,cognitive:2,narrative:1,onboarding:1,stop:'Save between hands',status:'playing',favorite:true,installed:['pc','deck'],deck:true,played:12,last:1,note:'A familiar run with no story to catch up on. Save when your time is up.'},
  {id:1055540,title:'A Short Hike',style:'Exploration',activities:['Explore'],min:10,ideal:25,setup:1,mechanical:1,cognitive:1,narrative:1,onboarding:1,stop:'Save whenever you like',status:'backlog',favorite:false,installed:['pc','deck'],deck:true,played:0,last:400,note:'A small change of scenery. Wander for a while and stop whenever you like.'},
  {id:632360,title:'Risk of Rain 2',style:'Action',activities:['Shoot','Multiplayer','Never-ending'],min:35,ideal:60,setup:3,mechanical:3,cognitive:2,narrative:1,onboarding:1,stop:'Between stages',status:'backlog',favorite:false,installed:['pc'],deck:true,played:6,last:190,note:'An energetic run from deeper in your library. It needs more room to breathe.'},
  {id:1245620,title:'Elden Ring',style:'RPG',activities:['Ride a horse','Explore'],min:30,ideal:90,setup:3,mechanical:3,cognitive:3,narrative:2,onboarding:1,stop:'At a Site of Grace',status:'backlog',favorite:false,installed:['pc'],deck:true,played:40,last:45,note:'A demanding return to a large world.'}
];
const defaults = {minutes:30,energy:1,activity:'Anything',brain:false,device:'pc',installed:false,favorites:false,scope:'any'};
const state = {context:{...defaults},phase:'setup',options:false,hand:[],seen:new Set(),dismissed:new Set(),saved:new Set(),excluded:new Set(),history:[],reshuffles:1,scenario:'normal',chosen:null};
const workspace = document.querySelector('#workspace');
const dialog = document.querySelector('#detail-dialog');
const energyNames = ['','Low','Medium','High'];
const energyIcons = ['', 'battery-low', 'battery-medium-01', 'battery-full'];
const activities = ['Anything','Shoot','Fly','Drive','Explore','Ride a horse','Play cards','Multiplayer','Never-ending','Weird'];
const activityIcons = {
  Shoot:'gun',
  Fly:'airplane-02',
  Drive:'car-04',
  Explore:'maps-search',
  'Ride a horse':'horse-saddle',
  'Play cards':'spades',
  Multiplayer:'user-group-03',
  'Never-ending':'infinity-square',
  Weird:'alien-02',
};
const visibleGames = games.filter(g=>!g.hidden);
const activityId = activity => activity.toLowerCase().replaceAll(' ','-');
let dialogTrigger = null;
let toastTimer;
let undoAction = null;
const byId = id => games.find(g=>g.id===Number(id));
const art = g => `../../Design/resources/images/${g.id}.jpg`;
const icon = (name,extra='') => `<span class="icon ${extra}" data-icon="${name}" aria-hidden="true"></span>`;
const button = (text,action,extra='',data='') => `<button class="button ${extra}" data-action="${action}" ${data}>${text}</button>`;
const formatTime = n => n===null?'No time limit':n===60?'1 hour':n===120?'2 hours':`${n} min`;
const statusName = s => ({playing:'Playing',backlog:'Backlog',want:'Want to play',paused:'Paused'}[s]);
function icons(root=document){root.querySelectorAll('[data-icon]').forEach(el=>el.style.setProperty('--icon',`url("../../Design/resources/icons-new/${el.dataset.icon}-stroke-rounded.svg")`));}
function notify(message,undo){clearTimeout(toastTimer);undoAction=undo||null;const el=document.querySelector('#toast');el.replaceChildren(document.createTextNode(message));if(undo){const b=document.createElement('button');b.textContent='Undo';b.dataset.action='undo-toast';el.append(b);}el.hidden=false;toastTimer=setTimeout(()=>{el.hidden=true;},undo?12000:5000);}
function record(g,action){state.history.unshift({id:g.id,action});}
function optionCount(){const c=state.context;return Number(c.installed)+Number(c.favorites)+Number(c.scope!=='any')+Number(c.device!=='pc');}
function exclusionReasons(g,c=state.context){
  const r=[];
  if(g.hidden)r.push('Hidden');
  if(c.activity!=='Anything'&&!g.activities.includes(c.activity))r.push('Activity');
  if(g.status==='paused')r.push('Paused status');
  if(state.excluded.has(g.id))r.push('Not interested');
  if(state.dismissed.has(g.id))r.push('Not now');
  if(c.minutes!==null&&g.min+g.setup>c.minutes)r.push('Available time');
  if(g.mechanical>c.energy)r.push('Energy');
  if(c.brain&&(g.cognitive>1||g.narrative>1||g.setup>3||(g.played===0&&g.onboarding>1)))r.push('Brain dead');
  if(c.installed&&!g.installed.includes(c.device))r.push('Installed only');
  if(c.favorites&&!g.favorite)r.push('Favorites only');
  if(c.scope==='unplayed'&&g.played>0)r.push('Unplayed only');
  if(c.scope==='playing'&&g.status!=='playing')r.push('Playing only');
  if(c.device==='deck'&&g.deck!==true)r.push('Setup evidence');
  return r;
}
function eligible(context=state.context){return games.filter(g=>exclusionReasons(g,context).length===0);}
function score(g){
  const c=state.context;
  return (c.activity==='Anything'?12:40)+(c.minutes===null?0:Math.max(0,15-Math.abs(g.ideal-c.minutes)/5))+(4-g.setup)*2+(g.favorite?5:0)+(g.status==='playing'?4:0)+(g.last>90?3:0);
}
function selectHand(excludeSeen=false){
  const pool=eligible().filter(g=>!excludeSeen||!state.seen.has(g.id)).sort((a,b)=>score(b)-score(a)||a.id-b.id);
  const hand=[];
  // Diversity only wins among close fits; a different genre cannot outweigh context.
  while(pool.length&&hand.length<3){
    const best=pool[0],different=pool.findIndex(g=>score(g)>=score(best)-8&&!hand.some(p=>p.style===g.style));
    hand.push(pool.splice(different<0?0:different,1)[0]);
  }
  return hand;
}
function contextStrip(){const c=state.context;return `<div class="context-strip"><div class="context-values"><span class="context-value">${icon(c.minutes===null?'infinity-01':'time-04')}${formatTime(c.minutes)}</span><span class="context-value">${icon(energyIcons[c.energy])}${energyNames[c.energy]} energy</span><span class="context-value">${activityIcons[c.activity]?icon(activityIcons[c.activity]):''}${c.activity}</span>${c.brain?`<span class="context-value">${icon('brain-02')}Brain dead</span>`:''}<span class="context-value">${c.device==='pc'?'PC':'Steam Deck'}</span>${c.installed?'<span class="context-value">Installed</span>':''}${c.favorites?'<span class="context-value">Favorites</span>':''}${c.scope!=='any'?`<span class="context-value">${c.scope==='unplayed'?'Unplayed':'Playing'}</span>`:''}</div><button class="button context-edit" data-action="edit">Edit</button></div>`;}
function setupView(){
  const c=state.context;
  return `<div class="intro"><div class="eyebrow">Play now</div><h1 tabindex="-1" id="view-heading">What fits right now?</h1><p>A little context. Three games from your library.</p></div>
  <div class="setup-layout"><form class="setup-form" id="setup-form">
    <fieldset class="control-section"><legend><span class="section-number">01</span>How much time do you have?</legend><div class="segments" aria-label="Available time">${[15,30,45,60,120].map(n=>`<button type="button" id="time-${n}" data-action="time" data-value="${n}" aria-pressed="${c.minutes===n}">${formatTime(n)}</button>`).join('')}<button type="button" id="time-unlimited" data-action="time" data-value="unlimited" aria-pressed="${c.minutes===null}" aria-label="No time limit" title="No time limit">${icon('infinity-01')}</button></div></fieldset>
    <fieldset class="control-section"><legend><span class="section-number">02</span>How much energy do you have?</legend><div class="segments" aria-label="Energy">${[1,2,3].map(n=>`<button type="button" id="energy-${n}" data-action="energy" data-value="${n}" aria-pressed="${c.energy===n}">${icon(energyIcons[n])}${energyNames[n]}</button>`).join('')}</div></fieldset>
    <fieldset class="control-section"><legend><span class="section-number">03</span>What do you feel like doing? <span class="optional">Optional</span></legend><div class="activities">${activities.map(activity=>`<button type="button" class="chip" id="activity-${activityId(activity)}" data-action="activity" data-value="${activity}" aria-pressed="${c.activity===activity}"${activity==='Never-ending'?' title="Another run: roguelikes and replayable loops"':''}>${activityIcons[activity]?icon(activityIcons[activity]):''}${activity}</button>`).join('')}</div></fieldset>
    <div class="brain-control">${icon('brain-02','brain-icon')}<div class="brain-copy"><strong id="brain-label">Brain dead</strong><p id="brain-help">Less thinking, less remembering.<br>Action is still on the table.</p></div><button type="button" class="switch" role="switch" id="brain-switch" aria-checked="${c.brain}" aria-labelledby="brain-label" aria-describedby="brain-help" data-action="brain"></button></div>
    <details class="options" ${state.options?'open':''}><summary>${icon('chevron-right','small')}More options ${optionCount()?`<span class="option-count">${optionCount()}</span>`:''}</summary><div class="option-fields"><label class="select-row">Playing on <select id="device"><option value="pc" ${c.device==='pc'?'selected':''}>PC</option><option value="deck" ${c.device==='deck'?'selected':''}>Steam Deck</option></select></label><label class="select-row">Choose from <select id="scope"><option value="any">All eligible games</option><option value="unplayed" ${c.scope==='unplayed'?'selected':''}>Unplayed</option><option value="playing" ${c.scope==='playing'?'selected':''}>Playing</option></select></label><label class="check-row"><input type="checkbox" id="installed" ${c.installed?'checked':''}>Installed on this setup only</label><label class="check-row"><input type="checkbox" id="favorites" ${c.favorites?'checked':''}>Favorites only</label></div></details>
    <button type="submit" class="button primary deal-button">${icon('cards-02')}Deal me 3</button><p class="form-footnote">Up to three choices. One optional reshuffle.</p>
  </form><div class="preview-stage" aria-hidden="true"><div class="card-fan"><div class="card-back"></div><div class="card-back"></div><div class="card-back">${icon('cards-02')}<em>GAMESYNC</em></div></div><h2>Your library, down to three.</h2><p>A familiar favorite, a different direction,<br>or something you forgot you had.</p><span class="preview-caption">${icon('game-controller-03','small')}${visibleGames.length} visible games in this demo library</span></div></div>`;
}
function reason(g){const c=state.context;const opening=c.brain?'Little to remember, with a quick start. ':c.activity==='Never-ending'?'A fresh run whenever you feel like it. ':'';return opening+g.note;}
function roleFor(g,i){if(i===0)return 'Closest fit';if(g.last>=90)return 'Worth another look';return 'Another good fit';}
function cover(g,missing=false){return missing?`<div class="cover-image missing-cover">${icon('game-controller-03')}<strong>${g.title}</strong><small>Cover unavailable</small></div>`:`<img class="cover-image" src="${art(g)}" alt="${g.title} cover" width="600" height="900">`;}
function cardView(g,i){
  if(state.dismissed.has(g.id)||state.excluded.has(g.id))return `<article class="game-card"><div class="card-role"><span class="number">0${i+1}</span>Set aside</div><div class="dismissed-card">${icon('cards-02')}<h3>${g.title}</h3><p>${state.excluded.has(g.id)?'Removed from future recommendations.':'Not for this session.'}</p>${button('Undo',state.excluded.has(g.id)?'unexclude':'undo-dismiss','',`data-id="${g.id}"`)}</div></article>`;
  const installed=g.installed.includes(state.context.device);
  return `<article class="game-card"><div class="card-role"><span class="number">0${i+1}</span>${roleFor(g,i)}</div><button class="cover-button" aria-label="Details for ${g.title}" data-action="details" data-id="${g.id}">${cover(g,state.scenario==='missing'&&i===0)}<span class="cover-hint">${icon('info')}</span></button><div class="card-info"><h3>${g.title}</h3><div class="game-meta"><span>${icon('time-04')}${g.min}–${g.ideal} min</span><span>${installed?'Installed':'Not installed'}</span></div><p class="card-reason">${reason(g)}</p><div class="card-actions">${button(installed?'Play':'Choose this', 'play','primary',`data-id="${g.id}" aria-label="${installed?'Play':'Choose'} ${g.title}"`)}${button(icon('clock-fading'),'save','icon-button',`data-id="${g.id}" aria-label="${state.saved.has(g.id)?'Unsave':'Save for later:'} ${g.title}" aria-pressed="${state.saved.has(g.id)}" title="${state.saved.has(g.id)?'Remove saved pick':'Save for later'}"`)}</div><div class="card-secondary"><button class="quiet small-button" data-action="dismiss" data-id="${g.id}">Not now</button><button class="quiet small-button" data-action="details" data-id="${g.id}">Why this game?</button></div></div></article>`;
}
function handView(){
  const remaining=eligible().filter(g=>!state.seen.has(g.id)).length;
  const visible=state.hand.filter(g=>!state.dismissed.has(g.id)&&!state.excluded.has(g.id)).length;
  return `<div class="result-heading"><div><h1 tabindex="-1" id="view-heading">Your hand for right now.</h1><p>${visible?`${visible===3?'Three':visible===2?'Two':'One'} ${visible===1?'game':'games'} to choose from. Start with the one that catches your eye.`:'Nothing left in this hand. Undo a pass or try your reshuffle.'}</p></div><span class="result-count">${eligible().length} eligible · ${visibleGames.length} in library</span></div>${contextStrip()}<div class="hand">${state.hand.map(cardView).join('')}</div><div class="hand-footer"><p>${state.reshuffles?'Not quite right? You have one reshuffle.':'That was your reshuffle. Choose a game, or edit your preferences.'}</p><div class="button-row">${button(icon('refresh-04')+'Reshuffle','reshuffle','',`${!state.reshuffles||!remaining?'disabled':''} title="${!state.reshuffles?'Reshuffle used':!remaining?'No unseen games fit these settings':'Deal a different hand'}"`)}</div></div>${state.reshuffles&&!remaining?'<p class="form-footnote">No unseen games fit these settings. You can edit your context.</p>':''}`;
}
function emptyView(unknown=false){
  const c=state.context;
  const restrictions=c.brain?['Brain dead',`${energyNames[c.energy]} energy`,formatTime(c.minutes)]:[`${energyNames[c.energy]} energy`,formatTime(c.minutes)];
  if(c.activity!=='Anything')restrictions.push(c.activity);
  if(c.installed)restrictions.push('Installed only');if(c.favorites)restrictions.push('Favorites only');if(c.scope!=='any')restrictions.push(c.scope==='unplayed'?'Unplayed only':'Playing only');
  const relaxations=[['activity','Anything','Try any activity'],['favorites',false,'Include non-favorites'],['scope','any','Include played games'],['installed',false,'Include uninstalled games'],['brain',false,'Turn Brain dead off'],['minutes',60,'Try 1 hour']].filter(([key,value])=>c[key]!==value&&(key!=='minutes'||(c.minutes!==null&&c.minutes<60))).map(([key,value,label])=>({key,value,label,count:eligible({...c,[key]:value}).length})).filter(x=>x.count>0).slice(0,2);
  return `<div class="eyebrow">Play now</div>${contextStrip()}<div class="empty-state">${icon(unknown?'info':'cards-02')}<h2 tabindex="-1" id="view-heading">${unknown?'A little more context about your games.':'No games fit this hand yet.'}</h2><p>${unknown?'Session length and effort are missing for these sample games. Review their profiles before treating them as a match.':'Your current settings leave no eligible games. Adjust the limits below, then deal again.'}</p><div class="filter-list">${(unknown?['Session length unknown','Effort unknown']:restrictions).map(x=>`<span>${x}</span>`).join('')}</div><div class="button-row">${button(unknown?'Review sample profiles':'Adjust my context',unknown?'profiles':'edit','primary')}${unknown?'':relaxations.map(x=>button(`${x.label} (${x.count})`,'relax','',`data-key="${x.key}" data-value="${x.value}"`)).join('')}</div>${button('New session','new-session','quiet small-button','style="margin-top:18px"')}</div>`;
}
function loadingView(){return `<div class="eyebrow">Play now</div><h1 tabindex="-1" id="view-heading">Finding a small hand.</h1><p class="muted" style="margin-top:12px" role="status">Checking your saved game profiles…</p><div class="skeleton-hand" aria-hidden="true"><div class="skeleton"></div><div class="skeleton"></div><div class="skeleton"></div></div>${button('Cancel','edit')}<p class="form-footnote">Loading state preview. The normal demo selector runs instantly.</p>`;}
function chosenView(){const g=state.chosen;const error=state.scenario==='launch-error';const installed=g.installed.includes(state.context.device);return `<div class="eyebrow">Play now</div><div class="chosen">${cover(g)}<div><div class="eyebrow">${error?'Launch error preview':installed?'Your pick':'Saved as your pick'}</div><h1 tabindex="-1" id="view-heading">${error?'Couldn’t open the game.':g.title}</h1><p>${error?`Steam could not open ${g.title}. Your hand is still here, ready when you are.`:installed?`${state.context.minutes===null?'No time limit':formatTime(state.context.minutes)+' set aside'} for ${g.title}. This prototype stops here; no game was launched.`:`${g.title} is not installed on your selected setup. This prototype records your choice without installing or launching it.`}</p><div class="button-row">${error?button('Retry','retry','primary'):button('Back to my hand','back-hand','primary')}${error?button('Back to my hand','back-hand'):button('New session','new-session')}</div><p style="font-size:11px">${error?'Simulated error. No Steam request was sent.':'Your status and playtime stay unchanged.'}</p></div></div>`;}
function render(focusId){
  const active=document.activeElement;
  const restore=active?.closest('#workspace')&&active.dataset.action?{action:active.dataset.action,id:active.dataset.id}:null;
  workspace.innerHTML=state.phase==='setup'?setupView():state.phase==='hand'?handView():state.phase==='loading'?loadingView():state.phase==='chosen'?chosenView():emptyView(state.phase==='unknown');
  workspace.querySelector('.hand')?.style.setProperty('--hand-size',state.hand.length);
  icons(workspace);document.querySelector('#saved-count').textContent=state.saved.size;
  const form=document.querySelector('#setup-form');if(form)form.addEventListener('submit',e=>{
    e.preventDefault();
    deal();
  });
  const options=document.querySelector('.options');if(options)options.addEventListener('toggle',()=>state.options=options.open);
  if(focusId)document.getElementById(focusId)?.focus({preventScroll:true});
  else if(restore){
    const selector=`[data-action="${restore.action}"]${restore.id?`[data-id="${restore.id}"]`:''}`;
    const replacement=workspace.querySelector(selector)||(restore.id?workspace.querySelector(`[data-action="details"][data-id="${restore.id}"]`):null);
    replacement?.focus({preventScroll:true});
  }
  // Image failures keep card geometry and its usable title instead of a broken image icon.
  workspace.querySelectorAll('img').forEach(img=>img.addEventListener('error',()=>{const fallback=document.createElement('div');fallback.className='cover-image missing-cover';fallback.textContent=img.alt.replace(' cover','');img.replaceWith(fallback);}));
}
function go(phase){state.phase=phase;render('view-heading');}
function deal(reshuffle=false){
  if(state.scenario==='loading'){go('loading');return;}
  if(state.scenario==='empty'){go('empty');return;}
  if(state.scenario==='unknown'){go('unknown');return;}
  state.hand=selectHand(reshuffle);state.hand.forEach(g=>{state.seen.add(g.id);record(g,'Shown');});go(state.hand.length?'hand':'empty');
}
function showDialog(title,body){if(!dialog.open)dialogTrigger=document.activeElement;document.querySelector('#dialog-content').innerHTML=`<div class="dialog-header"><h2 id="dialog-title">${title}</h2><button class="quiet icon-button" data-action="close-dialog" aria-label="Close dialog">${icon('cancel-01')}</button></div><div class="dialog-body">${body}</div>`;icons(dialog);if(!dialog.open)dialog.showModal();else dialog.querySelector('[data-action="close-dialog"]').focus();}
function details(g){showDialog(g.title,`<div class="detail-top"><img src="${art(g)}" alt="${g.title} cover"><div><div class="eyebrow">Why this game?</div><p>${reason(g)}</p></div></div><dl class="detail-grid"><div><dt>Practical session</dt><dd>${g.min}–${g.ideal} minutes</dd></div><div><dt>Setup time</dt><dd>About ${g.setup} ${g.setup===1?'minute':'minutes'}</dd></div><div><dt>Thinking / mechanics</dt><dd>${energyNames[g.cognitive]} / ${energyNames[g.mechanical]}</dd></div><div><dt>Story to remember</dt><dd>${energyNames[g.narrative]}</dd></div><div><dt>Stopping point</dt><dd>${g.stop}</dd></div><div><dt>Your library</dt><dd>${statusName(g.status)} · ${g.played?`${g.played} hours played`:'Unplayed'}</dd></div></dl><p class="evidence-note">All values are invented demo profiles, including playtime and device fit. Production profiles will identify personal overrides and analysis separately. Unknown data will stay unknown.</p><div class="button-row">${button('Choose this game','play','primary',`data-id="${g.id}"`)}${button(state.saved.has(g.id)?'Remove saved pick':'Save for later','save','',`data-id="${g.id}"`)}<button class="quiet small-button" data-action="exclude" data-id="${g.id}">Not interested</button></div>`);}
function savedView(){const saved=games.filter(g=>state.saved.has(g.id));showDialog('Saved for later',saved.length?saved.map(g=>`<div class="list-row"><img src="${art(g)}" alt=""><div><strong>${g.title}</strong><p>Saved in this demo session</p></div><button class="quiet small-button" data-action="details" data-id="${g.id}">Details</button><button class="quiet small-button" data-action="remove-saved" data-id="${g.id}">Remove</button></div>`).join(''):'<p class="muted">Save a card from your hand to keep it here. Your library status stays the same.</p>');}
function recentView(){showDialog('Recent picks',`<p class="muted" style="font-size:12px;margin-bottom:18px">Actions from this demo session. Choosing a game does not confirm that you played it.</p>${state.history.length?state.history.slice(0,18).map(e=>{const g=byId(e.id);return `<div class="list-row"><img src="${art(g)}" alt=""><div><strong>${g.title}</strong><p>${e.action} · This session</p></div></div>`;}).join(''):'<p class="muted">Your first hand will appear here.</p>'}${state.excluded.size?`<h3 style="margin:24px 0 12px">Not interested</h3>${[...state.excluded].map(id=>`<div class="list-row"><div><strong>${byId(id).title}</strong><p>Excluded from recommendations in this demo</p></div><button class="quiet small-button" data-action="unexclude" data-id="${id}">Allow again</button></div>`).join('')}`:''}`);}
function resetSession(){state.hand=[];state.seen.clear();state.dismissed.clear();state.reshuffles=1;state.chosen=null;state.scenario='normal';document.querySelector('#scenario').value='normal';go('setup');}
function clearScenario(){state.scenario='normal';document.querySelector('#scenario').value='normal';}
function closeDialog(){dialog.close();}
dialog.addEventListener('close',()=>{
  const replacement=dialogTrigger?.dataset.id?workspace.querySelector(`[data-action="${dialogTrigger.dataset.action}"][data-id="${dialogTrigger.dataset.id}"]`):null;
  if(dialogTrigger?.isConnected)dialogTrigger.focus();else if(replacement)replacement.focus();else document.querySelector('#view-heading')?.focus();
});
dialog.addEventListener('click',e=>{if(e.target===dialog){const r=dialog.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)closeDialog();}});
document.addEventListener('click',e=>{
  const target=e.target.closest('[data-action]');if(!target||target.disabled)return;
  const a=target.dataset.action,g=byId(target.dataset.id),c=state.context;
  if(['time','energy','activity','brain'].includes(a)){
    if(a==='time')c.minutes=target.dataset.value==='unlimited'?null:Number(target.dataset.value);
    if(a==='energy')c.energy=Number(target.dataset.value);
    if(a==='activity')c.activity=target.dataset.value;
    if(a==='brain')c.brain=!c.brain;
    render(target.id);return;
  }
  if(a==='edit'){clearScenario();go('setup');}
  if(a==='back-hand'){clearScenario();go(state.hand.length?'hand':'setup');}
  if(a==='new-session')resetSession();
  if(a==='reshuffle'&&state.reshuffles&&eligible().some(x=>!state.seen.has(x.id))){state.reshuffles--;deal(true);}
  if(a==='details')details(g);
  if(a==='close-dialog')closeDialog();
  if(a==='save'){const has=state.saved.has(g.id);has?state.saved.delete(g.id):state.saved.add(g.id);if(!has)record(g,'Saved for later');render();if(dialog.open)details(g);notify(has?'Removed from saved picks.':`${g.title} saved for later.`);}
  if(a==='remove-saved'){state.saved.delete(g.id);render();savedView();}
  if(a==='dismiss'){state.dismissed.add(g.id);record(g,'Not now');render();notify(`${g.title} set aside for this session.`,()=>{state.dismissed.delete(g.id);render();});workspace.querySelector(`[data-action="undo-dismiss"][data-id="${g.id}"]`)?.focus();}
  if(a==='undo-dismiss'){state.dismissed.delete(g.id);render();}
  if(a==='exclude'){state.excluded.add(g.id);record(g,'Not interested');closeDialog();render();notify(`${g.title} excluded from recommendations.`,()=>{state.excluded.delete(g.id);render();});}
  if(a==='unexclude'){state.excluded.delete(g.id);render();if(dialog.open)recentView();}
  if(a==='play'){state.chosen=g;record(g,'Chosen (demo only)');if(dialog.open)closeDialog();go('chosen');}
  if(a==='retry'){clearScenario();go('chosen');notify('Retry preview complete. No game was launched.');}
  if(a==='undo-toast'){undoAction?.();document.querySelector('#toast').hidden=true;}
  if(a==='relax'){const value=target.dataset.value;c[target.dataset.key]=value==='false'?false:target.dataset.key==='minutes'?Number(value):value;clearScenario();deal();}
  if(a==='profiles')showDialog('Profiles need a review',`<p class="muted">These sample games need session length and effort values before they can be matched reliably.</p><p class="evidence-note">The native version will let you edit your own values or review suggested profiles. Analysis will never replace your overrides.</p>${button('Use complete demo profiles','complete-profiles','primary')}`);
  if(a==='complete-profiles'){closeDialog();clearScenario();deal();}
});
document.addEventListener('change',e=>{
  const t=e.target,c=state.context;
  if(t.id==='device')c.device=t.value;
  if(t.id==='scope')c.scope=t.value;
  if(t.id==='installed')c.installed=t.checked;
  if(t.id==='favorites')c.favorites=t.checked;
  if(['device','scope','installed','favorites'].includes(t.id))render(t.id);
  if(t.id==='scenario'){
    state.scenario=t.value;
    if(t.value==='normal')go('setup');
    else if(t.value==='loading')go('loading');
    else if(t.value==='empty'||t.value==='unknown')go(t.value);
    else if(t.value==='missing'){if(!state.hand.length){state.context={...defaults};state.hand=selectHand();state.hand.forEach(g=>state.seen.add(g.id));}go('hand');}
    else if(t.value==='launch-error'){if(!state.hand.length){state.context={...defaults};state.hand=selectHand();}state.chosen=state.hand[0]||games[1];go('chosen');}
  }
});
document.querySelector('#play-nav').addEventListener('click',()=>go(state.hand.length?'hand':'setup'));
document.querySelector('#saved-button').addEventListener('click',savedView);
document.querySelector('#recent-button').addEventListener('click',recentView);
document.querySelector('#theme-button').addEventListener('click',e=>{const dark=document.documentElement.dataset.theme!=='dark';document.documentElement.dataset.theme=dark?'dark':'light';e.target.textContent=dark?'Light':'Dark';e.target.setAttribute('aria-label',`Switch to ${dark?'light':'dark'} appearance`);});
document.querySelector('#reset-button').addEventListener('click',()=>{state.context={...defaults};state.options=false;state.saved.clear();state.excluded.clear();state.history=[];resetSession();notify('Demo reset.');});
document.addEventListener('keydown',()=>document.documentElement.classList.add('keyboard'));
document.addEventListener('pointerdown',()=>document.documentElement.classList.remove('keyboard'));
document.querySelector('#library-count').textContent=visibleGames.length;
icons();render();
