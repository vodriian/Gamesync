import { sql } from "drizzle-orm";
import { pgTable, text, varchar, integer, timestamp, jsonb } from "drizzle-orm/pg-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod";

// User configuration table
export const userConfig = pgTable("user_config", {
  id: varchar("id").primaryKey().default(sql`gen_random_uuid()`),
  steamKey: text("steam_key"),
  steamId: text("steam_id"),
  notionToken: text("notion_token"),
  notionDatabaseId: text("notion_database_id"),
  craftApiUrl: text("craft_api_url"),
  craftCollectionId: text("craft_collection_id"),
  updatedAt: timestamp("updated_at").defaultNow(),
});

// Custom column configurations
export const customColumns = pgTable("custom_columns", {
  id: varchar("id").primaryKey().default(sql`gen_random_uuid()`),
  name: text("name").notNull(),
  type: text("type").notNull(), // text, select, multi-select, number, date, url
  options: jsonb("options"), // for select/multi-select types
  position: integer("position").notNull(),
  visible: integer("visible").notNull().default(1), // SQLite-style boolean (1 = true, 0 = false)
  isSystem: integer("is_system").notNull().default(0), // 1 = system column, cannot be deleted
  createdAt: timestamp("created_at").defaultNow(),
});

// Cached game data
export const games = pgTable("games", {
  id: varchar("id").primaryKey(), // appId from Steam
  name: text("name").notNull(),
  coverImage: text("cover_image"),
  headerImage: text("header_image"),
  description: text("description"),
  // ProtonDB data
  protonTier: text("proton_tier"),
  protonConfidence: text("proton_confidence"),
  // Steam review data
  steamRating: text("steam_rating"),
  ratingTotal: integer("rating_total"),
  ratingPositivePct: integer("rating_positive_pct"),
  // User status
  status: text("status"),
  // Custom properties
  customProperties: jsonb("custom_properties"),
  lastSynced: timestamp("last_synced"),
  createdAt: timestamp("created_at").defaultNow(),
});

// Insert schemas
export const insertUserConfigSchema = createInsertSchema(userConfig).omit({
  id: true,
  updatedAt: true,
});

export const insertCustomColumnSchema = createInsertSchema(customColumns).omit({
  id: true,
  createdAt: true,
});

export const insertGameSchema = createInsertSchema(games).omit({
  createdAt: true,
  lastSynced: true,
});

// Types
export type InsertUserConfig = z.infer<typeof insertUserConfigSchema>;
export type UserConfig = typeof userConfig.$inferSelect;

export type InsertCustomColumn = z.infer<typeof insertCustomColumnSchema>;
export type CustomColumn = typeof customColumns.$inferSelect;

export type InsertGame = z.infer<typeof insertGameSchema>;
export type Game = typeof games.$inferSelect;

// Default system columns with select options
export const DEFAULT_SYSTEM_COLUMNS = [
  {
    name: "Tier",
    type: "select",
    options: [
      "06 native 🌿",
      "05 platinum 👑",
      "04 gold 🥇",
      "03 silver 🥈",
      "02 bronze 🥉",
      "01 borked 💀",
      "00 pending ⏳",
      "00 unknown ❓"
    ],
    position: 0,
    visible: 1,
    isSystem: 1,
  },
  {
    name: "Confidence",
    type: "select",
    options: [
      "03 high ✅",
      "02 medium 🟡",
      "01 low ⚠️",
      "00 unknown ❓"
    ],
    position: 1,
    visible: 1,
    isSystem: 1,
  },
  {
    name: "Rating",
    type: "select",
    options: [
      "09 overwhelmingly positive 😍",
      "08 very positive 🙂",
      "07 positive 👍",
      "06 mostly positive 🙂‍↕️",
      "05 mixed 😐",
      "04 mostly negative 👎",
      "03 negative 😕",
      "02 very negative 😬",
      "01 overwhelmingly negative 💣",
      "00 no reviews 0️⃣"
    ],
    position: 2,
    visible: 1,
    isSystem: 1,
  },
  {
    name: "RatingTotal",
    type: "number",
    options: null,
    position: 3,
    visible: 1,
    isSystem: 1,
  },
  {
    name: "RatingPositivePct",
    type: "number",
    options: null,
    position: 4,
    visible: 1,
    isSystem: 1,
  },
  {
    name: "Status",
    type: "select",
    options: [
      "04 completed ✅",
      "03 playing 🎮",
      "02 backlog 📚",
      "01 dropped 🧹",
      "00 wishlist ⭐"
    ],
    position: 5,
    visible: 1,
    isSystem: 1,
  },
];
