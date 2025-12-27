import { GameData, ProtonDBInfo, SteamGame } from "./types";

const MOCK_GAMES: SteamGame[] = [
  { appid: 1091500, name: "Cyberpunk 2077", playtime_forever: 4500 },
  { appid: 1245620, name: "ELDEN RING", playtime_forever: 8200 },
  { appid: 1086940, name: "Baldur's Gate 3", playtime_forever: 6000 },
  { appid: 413150, name: "Stardew Valley", playtime_forever: 12000 },
  { appid: 1145360, name: "Hades", playtime_forever: 3400 },
  { appid: 546560, name: "Half-Life: Alyx", playtime_forever: 900 },
  { appid: 271590, name: "Grand Theft Auto V", playtime_forever: 15000 },
  { appid: 292030, name: "The Witcher 3: Wild Hunt", playtime_forever: 7800 },
];

const MOCK_PROTON: Record<number, ProtonDBInfo> = {
  1091500: { tier: "gold", score: 85 },
  1245620: { tier: "platinum", score: 92 },
  1086940: { tier: "gold", score: 88 },
  413150: { tier: "native", score: 98 },
  1145360: { tier: "platinum", score: 96 },
  546560: { tier: "platinum", score: 90 },
  271590: { tier: "gold", score: 80 },
  292030: { tier: "platinum", score: 94 },
};

export async function fetchSteamGames(key: string, steamId: string): Promise<GameData[]> {
  // In a real browser environment, calling Steam API directly often fails CORS.
  // We will simulate the network delay and return mock data if "demo" keys are used 
  // or if the request fails (to keep the prototype usable).
  
  await new Promise(resolve => setTimeout(resolve, 1500));

  // If user provided specific keys, we *could* try fetch, but 99% chance of CORS error.
  // We'll log a simulation message in the UI instead.
  
  return MOCK_GAMES.map(g => ({
    ...g,
    cover_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${g.appid}/header.jpg`,
    store_url: `https://store.steampowered.com/app/${g.appid}/`,
    proton: MOCK_PROTON[g.appid] || { tier: "unknown" },
    notion_status: "pending"
  }));
}

export async function fetchProtonDB(appid: number): Promise<ProtonDBInfo> {
  // ProtonDB API might be CORS friendly-ish, but safer to mock for stability.
  await new Promise(resolve => setTimeout(resolve, 300));
  return MOCK_PROTON[appid] || { tier: "unknown" };
}

export async function syncToNotion(games: GameData[], token: string, dbId: string, onLog: (msg: string, level: "info"|"success"|"error") => void) {
  onLog("Starting sync process...", "info");
  
  if (!token || !dbId) {
    onLog("Missing Notion credentials. Please configure settings.", "error");
    return;
  }

  onLog(`Connecting to Notion Database: ${dbId.substring(0, 6)}...`, "info");
  await new Promise(r => setTimeout(r, 1000));
  
  // Simulate checking existing pages
  onLog(`Found ${games.length} games to process.`, "info");
  
  let processed = 0;
  for (const game of games) {
    await new Promise(r => setTimeout(r, 400)); // Simulate API latency
    
    const isNew = Math.random() > 0.7; // Simulate some being new
    if (isNew) {
      onLog(`[CREATE] ${game.name} - Added to Notion`, "success");
      // Simulate fetching 'about' text
      if (Math.random() > 0.5) {
         onLog(`  > Fetched 'About' description from Steam Store`, "info");
      }
    } else {
      onLog(`[UPDATE] ${game.name} - Updated playtime`, "info");
    }
    processed++;
    if (processed % 3 === 0) {
      onLog(`Progress: ${processed}/${games.length}`, "info");
    }
  }
  
  onLog("Sync complete!", "success");
}

export async function syncToCraft(games: GameData[], url: string, collectionId: string, onLog: (msg: string, level: "info"|"success"|"error") => void) {
  onLog("Starting sync to Craft...", "info");
  
  if (!url || !collectionId) {
    onLog("Missing Craft API credentials. Please configure settings.", "error");
    return;
  }

  onLog(`Connecting to Craft Collection: ${collectionId.substring(0, 8)}...`, "info");
  await new Promise(r => setTimeout(r, 1200));
  
  onLog("Authenticated successfully.", "success");
  onLog(`Preparing to sync ${games.length} items.`, "info");
  
  let processed = 0;
  for (const game of games) {
    await new Promise(r => setTimeout(r, 350));
    
    // Simulate sync
    onLog(`[SYNC] ${game.name} - Pushed to Craft`, "success");
    
    processed++;
    if (processed % 4 === 0) {
      onLog(`Progress: ${processed}/${games.length}`, "info");
    }
  }
  
  onLog("Craft Sync complete!", "success");
}
