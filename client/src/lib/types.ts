export interface SteamGame {
  appid: number;
  name: string;
  playtime_forever: number; // minutes
  img_icon_url?: string;
  img_logo_url?: string;
  // Augmented data
  cover_url?: string;
  store_url?: string;
}

export interface ProtonDBInfo {
  tier: "platinum" | "gold" | "silver" | "bronze" | "borked" | "native" | "pending" | "unknown";
  score?: number;
  total?: number;
  trendingTier?: string;
}

export interface GameData extends SteamGame {
  proton?: ProtonDBInfo;
  review_summary?: string;
  review_score?: number; // 0-100
  review_total?: number;
  notion_status?: "synced" | "pending" | "error";
  last_synced?: string;
}

export interface AppConfig {
  steamKey: string;
  steamId: string;
  notionToken: string;
  notionDbId: string;
}

export interface LogEntry {
  id: string;
  timestamp: Date;
  level: "info" | "success" | "warning" | "error";
  message: string;
}
