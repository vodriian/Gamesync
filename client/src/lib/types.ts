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
  description?: string;
  header_url?: string;
  customProperties?: Record<string, any>;
  
  // Dynamic user columns
  [key: string]: any;
}

export interface AppConfig {
  steamKey: string;
  steamId: string;
  notionToken: string;
  notionDbId: string;
  craftUrl?: string;
  craftCollectionId?: string;
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
  { id: "appid", label: "App ID", type: "number", visible: false, system: true },
  { id: "playtime_forever", label: "Playtime", type: "number", visible: true, system: true },
  { id: "proton_tier", label: "Tier", type: "select", visible: true, system: true, options: [
    "06 native 🌿", "05 platinum 👑", "04 gold 🥇", "03 silver 🥈", 
    "02 bronze 🥉", "01 borked 💥", "00 pending ⏳", "07 unknown ❓"
  ]},
  { id: "proton_confidence", label: "Confidence", type: "select", visible: true, system: true, options: [
    "03 high ✅", "02 medium 🟡", "01 low ⚠️", "00 unknown ❓"
  ]},
  { id: "steam_rating", label: "Rating", type: "select", visible: true, system: true, options: [
    "09 overwhelmingly positive 😍", "08 very positive 🙂", "07 positive 👍",
    "06 mostly positive 🙂‍↕️", "05 mixed 😐", "04 mostly negative 👎",
    "03 negative 😕", "02 very negative 😬", "01 overwhelmingly negative 💣", "00 no reviews 0️⃣"
  ]},
  { id: "notion_status", label: "Sync", type: "status", visible: true, system: true },
];
