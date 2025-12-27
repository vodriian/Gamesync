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
  createdAt: timestamp("created_at").defaultNow(),
});

// Cached game data
export const games = pgTable("games", {
  id: varchar("id").primaryKey(), // appId from Steam
  name: text("name").notNull(),
  coverImage: text("cover_image"),
  headerImage: text("header_image"),
  protonRating: text("proton_rating"),
  protonTier: text("proton_tier"),
  description: text("description"),
  customProperties: jsonb("custom_properties"), // User-defined property values
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
