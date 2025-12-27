import { eq } from "drizzle-orm";
import { db } from "./db";
import {
  type UserConfig,
  type InsertUserConfig,
  type CustomColumn,
  type InsertCustomColumn,
  type Game,
  type InsertGame,
  userConfig,
  customColumns,
  games,
} from "@shared/schema";

export interface IStorage {
  // User Configuration
  getUserConfig(): Promise<UserConfig | undefined>;
  upsertUserConfig(config: InsertUserConfig): Promise<UserConfig>;

  // Custom Columns
  getCustomColumns(): Promise<CustomColumn[]>;
  createCustomColumn(column: InsertCustomColumn): Promise<CustomColumn>;
  updateCustomColumn(id: string, column: Partial<InsertCustomColumn>): Promise<CustomColumn | undefined>;
  deleteCustomColumn(id: string): Promise<void>;
  reorderColumns(columnIds: string[]): Promise<void>;

  // Games
  getGames(): Promise<Game[]>;
  getGame(id: string): Promise<Game | undefined>;
  upsertGame(game: InsertGame): Promise<Game>;
  updateGameProperties(id: string, properties: any): Promise<Game | undefined>;
  deleteGame(id: string): Promise<void>;
}

export class DatabaseStorage implements IStorage {
  // User Configuration
  async getUserConfig(): Promise<UserConfig | undefined> {
    const configs = await db.select().from(userConfig).limit(1);
    return configs[0];
  }

  async upsertUserConfig(config: InsertUserConfig): Promise<UserConfig> {
    // Check if config exists
    const existing = await this.getUserConfig();
    
    if (existing) {
      // Update existing config
      const updated = await db
        .update(userConfig)
        .set({ ...config, updatedAt: new Date() })
        .where(eq(userConfig.id, existing.id))
        .returning();
      return updated[0];
    } else {
      // Insert new config
      const inserted = await db.insert(userConfig).values(config).returning();
      return inserted[0];
    }
  }

  // Custom Columns
  async getCustomColumns(): Promise<CustomColumn[]> {
    return await db.select().from(customColumns).orderBy(customColumns.position);
  }

  async createCustomColumn(column: InsertCustomColumn): Promise<CustomColumn> {
    const inserted = await db.insert(customColumns).values(column).returning();
    return inserted[0];
  }

  async updateCustomColumn(id: string, column: Partial<InsertCustomColumn>): Promise<CustomColumn | undefined> {
    const updated = await db
      .update(customColumns)
      .set(column)
      .where(eq(customColumns.id, id))
      .returning();
    return updated[0];
  }

  async deleteCustomColumn(id: string): Promise<void> {
    await db.delete(customColumns).where(eq(customColumns.id, id));
  }

  async reorderColumns(columnIds: string[]): Promise<void> {
    // Update positions sequentially
    for (let i = 0; i < columnIds.length; i++) {
      await db
        .update(customColumns)
        .set({ position: i })
        .where(eq(customColumns.id, columnIds[i]));
    }
  }

  // Games
  async getGames(): Promise<Game[]> {
    return await db.select().from(games).orderBy(games.name);
  }

  async getGame(id: string): Promise<Game | undefined> {
    const results = await db.select().from(games).where(eq(games.id, id)).limit(1);
    return results[0];
  }

  async upsertGame(game: InsertGame): Promise<Game> {
    const existing = await this.getGame(game.id);
    
    if (existing) {
      // Update existing game
      const updated = await db
        .update(games)
        .set({ ...game, lastSynced: new Date() })
        .where(eq(games.id, game.id))
        .returning();
      return updated[0];
    } else {
      // Insert new game
      const inserted = await db.insert(games).values(game).returning();
      return inserted[0];
    }
  }

  async updateGameProperties(id: string, properties: any): Promise<Game | undefined> {
    const updated = await db
      .update(games)
      .set({ customProperties: properties, lastSynced: new Date() })
      .where(eq(games.id, id))
      .returning();
    return updated[0];
  }

  async deleteGame(id: string): Promise<void> {
    await db.delete(games).where(eq(games.id, id));
  }
}

export const storage = new DatabaseStorage();
