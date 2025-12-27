import type { GameData } from "./types";
import type { UserConfig, CustomColumn, Game } from "@shared/schema";

const API_BASE = "/api";

// Configuration API
export async function getConfig(): Promise<Partial<UserConfig>> {
  const response = await fetch(`${API_BASE}/config`);
  if (!response.ok) {
    throw new Error("Failed to fetch configuration");
  }
  return response.json();
}

export async function saveConfig(config: Partial<UserConfig>): Promise<UserConfig> {
  const response = await fetch(`${API_BASE}/config`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to save configuration");
  }
  
  return response.json();
}

// Custom Columns API
export async function getColumns(): Promise<CustomColumn[]> {
  const response = await fetch(`${API_BASE}/columns`);
  if (!response.ok) {
    throw new Error("Failed to fetch columns");
  }
  return response.json();
}

export async function createColumn(column: Omit<CustomColumn, "id" | "createdAt">): Promise<CustomColumn> {
  const response = await fetch(`${API_BASE}/columns`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(column),
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to create column");
  }
  
  return response.json();
}

export async function updateColumn(id: string, updates: Partial<CustomColumn>): Promise<CustomColumn> {
  const response = await fetch(`${API_BASE}/columns/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(updates),
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to update column");
  }
  
  return response.json();
}

export async function deleteColumn(id: string): Promise<void> {
  const response = await fetch(`${API_BASE}/columns/${id}`, {
    method: "DELETE",
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to delete column");
  }
}

export async function reorderColumns(columnIds: string[]): Promise<void> {
  const response = await fetch(`${API_BASE}/columns/reorder`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ columnIds }),
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || "Failed to reorder columns");
  }
}

// Games API
export async function getGames(): Promise<Game[]> {
  const response = await fetch(`${API_BASE}/games`);
  if (!response.ok) {
    throw new Error("Failed to fetch games");
  }
  return response.json();
}

export async function getGame(id: string): Promise<Game> {
  const response = await fetch(`${API_BASE}/games/${id}`);
  if (!response.ok) {
    throw new Error("Failed to fetch game");
  }
  return response.json();
}

// Sync APIs
export async function syncFromSteam(onLog?: (msg: string, level: "info" | "success" | "error") => void): Promise<{ success: boolean; gamesCount: number; message: string }> {
  if (onLog) onLog("Connecting to Steam API...", "info");
  
  try {
    const response = await fetch(`${API_BASE}/sync/steam`, {
      method: "POST",
    });
    
    const result = await response.json();
    
    if (!response.ok) {
      if (onLog) onLog(result.error || "Failed to sync from Steam", "error");
      throw new Error(result.error || "Failed to sync from Steam");
    }
    
    if (onLog) {
      onLog(`Fetched ${result.gamesCount} games from Steam`, "success");
      onLog("Enriched with ProtonDB compatibility data", "success");
      onLog("Steam library sync complete!", "success");
    }
    
    return result;
  } catch (error) {
    if (onLog) onLog(`Steam sync error: ${error instanceof Error ? error.message : 'Unknown error'}`, "error");
    throw error;
  }
}

export async function syncToNotion(
  games: GameData[],
  token: string,
  dbId: string,
  onLog: (msg: string, level: "info" | "success" | "error") => void
): Promise<void> {
  onLog("Starting sync to Notion...", "info");
  onLog(`Connecting to Notion Database: ${dbId.substring(0, 6)}...`, "info");
  
  const response = await fetch(`${API_BASE}/sync/notion`, {
    method: "POST",
  });
  
  const result = await response.json();
  
  if (!response.ok) {
    onLog(result.error || "Failed to sync to Notion", "error");
    throw new Error(result.error || "Failed to sync to Notion");
  }
  
  onLog(`Found ${result.synced + result.failed} games to process.`, "info");
  
  if (result.synced > 0) {
    onLog(`Successfully synced ${result.synced} games to Notion`, "success");
  }
  
  if (result.failed > 0) {
    onLog(`Failed to sync ${result.failed} games`, "error");
    result.errors?.slice(0, 3).forEach((error: string) => {
      onLog(`  - ${error}`, "error");
    });
  }
  
  if (result.success) {
    onLog("Sync complete!", "success");
  }
}

export async function syncToCraft(
  games: GameData[],
  url: string,
  collectionId: string,
  onLog: (msg: string, level: "info" | "success" | "error") => void
): Promise<void> {
  onLog("Starting sync to Craft...", "info");
  onLog(`Connecting to Craft Collection: ${collectionId.substring(0, 8)}...`, "info");
  
  const response = await fetch(`${API_BASE}/sync/craft`, {
    method: "POST",
  });
  
  const result = await response.json();
  
  if (!response.ok) {
    onLog(result.error || "Failed to sync to Craft", "error");
    throw new Error(result.error || "Failed to sync to Craft");
  }
  
  onLog("Authenticated successfully.", "success");
  onLog(`Preparing to sync ${result.synced + result.failed} items.`, "info");
  
  if (result.synced > 0) {
    onLog(`Successfully synced ${result.synced} games to Craft`, "success");
  }
  
  if (result.failed > 0) {
    onLog(`Failed to sync ${result.failed} games`, "error");
    result.errors?.slice(0, 3).forEach((error: string) => {
      onLog(`  - ${error}`, "error");
    });
  }
  
  if (result.success) {
    onLog("Craft Sync complete!", "success");
  }
}

// Extract tier name from formatted string (e.g., "05 platinum 👑" -> "platinum")
function extractTierName(formattedTier: string | null): "unknown" | "platinum" | "gold" | "silver" | "bronze" | "borked" | "native" | "pending" {
  if (!formattedTier) return "unknown";
  const parts = formattedTier.split(" ");
  if (parts.length >= 2) {
    const tierName = parts[1].toLowerCase();
    const validTiers = ["unknown", "platinum", "gold", "silver", "bronze", "borked", "native", "pending"] as const;
    if (validTiers.includes(tierName as any)) {
      return tierName as typeof validTiers[number];
    }
  }
  return "unknown";
}

// Convert backend Game format to frontend GameData format for compatibility
export function gameToGameData(game: Game): GameData {
  const tier = extractTierName(game.protonTier);
  
  return {
    appid: parseInt(game.id),
    name: game.name,
    playtime_forever: 0,
    cover_url: game.coverImage || "",
    header_url: game.headerImage || "",
    store_url: `https://store.steampowered.com/app/${game.id}/`,
    description: game.description || "",
    proton: {
      tier,
      score: game.ratingPositivePct || undefined,
    },
    notion_status: game.lastSynced ? "synced" : "pending",
    // Pass through formatted select values
    customProperties: {
      tier: game.protonTier || undefined,
      confidence: game.protonConfidence || undefined,
      rating: game.steamRating || undefined,
      status: game.status || undefined,
      ...(game.customProperties && typeof game.customProperties === 'object' ? game.customProperties : {}),
    },
  };
}
