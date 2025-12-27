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
  
  // Dynamic user columns
  [key: string]: any;
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

export interface ColumnConfig {
  id: string;
  label: string;
  type: "text" | "number" | "select" | "multi_select" | "date" | "url" | "status";
  visible: boolean;
  system?: boolean; // If true, cannot be deleted (e.g. Name, AppID)
  options?: string[]; // Allowed values for select/multi_select
}

export const DEFAULT_COLUMNS: ColumnConfig[] = [
  { id: "name", label: "Name", type: "text", visible: true, system: true },
  { id: "appid", label: "App ID", type: "number", visible: true, system: true },
  { id: "playtime_forever", label: "Playtime (min)", type: "number", visible: true, system: true },
  { id: "proton_tier", label: "Proton Tier", type: "select", visible: true, system: true, options: ["platinum", "gold", "silver", "bronze", "borked", "native"] },
  { id: "notion_status", label: "Sync Status", type: "status", visible: true, system: true },
  { id: "my_rating", label: "My Rating", type: "select", visible: false, system: false, options: ["⭐⭐⭐⭐⭐", "⭐⭐⭐⭐", "⭐⭐⭐", "⭐⭐", "⭐"] },
  { id: "comments", label: "Comments", type: "text", visible: false, system: false },
  { id: "tags", label: "Tags", type: "multi_select", visible: false, system: false, options: ["Finished", "Backlog", "Abandonware", "Replay"] },
];
