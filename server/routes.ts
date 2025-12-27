import type { Express } from "express";
import { createServer, type Server } from "http";
import { storage } from "./storage";
import { insertUserConfigSchema, insertCustomColumnSchema } from "@shared/schema";
import { z } from "zod";
import { steamService } from "./services/steam";
import { protondbService } from "./services/protondb";
import { notionService } from "./services/notion";

export async function registerRoutes(
  httpServer: Server,
  app: Express
): Promise<Server> {
  
  // Configuration Routes
  app.get("/api/config", async (req, res) => {
    try {
      const config = await storage.getUserConfig();
      res.json(config || {});
    } catch (error) {
      console.error("Error fetching config:", error);
      res.status(500).json({ error: "Failed to fetch configuration" });
    }
  });

  app.post("/api/config", async (req, res) => {
    try {
      const validatedConfig = insertUserConfigSchema.parse(req.body);
      const config = await storage.upsertUserConfig(validatedConfig);
      res.json(config);
    } catch (error) {
      if (error instanceof z.ZodError) {
        res.status(400).json({ error: "Invalid configuration data", details: error.errors });
      } else {
        console.error("Error saving config:", error);
        res.status(500).json({ error: "Failed to save configuration" });
      }
    }
  });

  // Custom Columns Routes
  app.get("/api/columns", async (req, res) => {
    try {
      const columns = await storage.getCustomColumns();
      res.json(columns);
    } catch (error) {
      console.error("Error fetching columns:", error);
      res.status(500).json({ error: "Failed to fetch columns" });
    }
  });

  app.post("/api/columns", async (req, res) => {
    try {
      const validatedColumn = insertCustomColumnSchema.parse(req.body);
      const column = await storage.createCustomColumn(validatedColumn);
      res.json(column);
    } catch (error) {
      if (error instanceof z.ZodError) {
        res.status(400).json({ error: "Invalid column data", details: error.errors });
      } else {
        console.error("Error creating column:", error);
        res.status(500).json({ error: "Failed to create column" });
      }
    }
  });

  app.patch("/api/columns/:id", async (req, res) => {
    try {
      const { id } = req.params;
      const column = await storage.updateCustomColumn(id, req.body);
      
      if (!column) {
        res.status(404).json({ error: "Column not found" });
        return;
      }
      
      res.json(column);
    } catch (error) {
      console.error("Error updating column:", error);
      res.status(500).json({ error: "Failed to update column" });
    }
  });

  app.delete("/api/columns/:id", async (req, res) => {
    try {
      const { id } = req.params;
      await storage.deleteCustomColumn(id);
      res.json({ success: true });
    } catch (error) {
      console.error("Error deleting column:", error);
      res.status(500).json({ error: "Failed to delete column" });
    }
  });

  app.post("/api/columns/reorder", async (req, res) => {
    try {
      const { columnIds } = req.body;
      
      if (!Array.isArray(columnIds)) {
        res.status(400).json({ error: "columnIds must be an array" });
        return;
      }
      
      await storage.reorderColumns(columnIds);
      res.json({ success: true });
    } catch (error) {
      console.error("Error reordering columns:", error);
      res.status(500).json({ error: "Failed to reorder columns" });
    }
  });

  // Games Routes
  app.get("/api/games", async (req, res) => {
    try {
      const games = await storage.getGames();
      res.json(games);
    } catch (error) {
      console.error("Error fetching games:", error);
      res.status(500).json({ error: "Failed to fetch games" });
    }
  });

  app.get("/api/games/:id", async (req, res) => {
    try {
      const { id } = req.params;
      const game = await storage.getGame(id);
      
      if (!game) {
        res.status(404).json({ error: "Game not found" });
        return;
      }
      
      res.json(game);
    } catch (error) {
      console.error("Error fetching game:", error);
      res.status(500).json({ error: "Failed to fetch game" });
    }
  });

  // Steam Sync Routes
  app.post("/api/sync/steam", async (req, res) => {
    try {
      const config = await storage.getUserConfig();
      
      if (!config || !config.steamKey || !config.steamId) {
        res.status(400).json({ error: "Steam API key and Steam ID are required" });
        return;
      }
      
      // Fetch games from Steam
      const steamGames = await steamService.getGamesWithDetails(config.steamKey, config.steamId);
      
      // Enrich with ProtonDB data
      const enrichedGames = await protondbService.enrichGamesWithProtonData(steamGames);
      
      // Save to database
      for (const game of enrichedGames) {
        await storage.upsertGame({
          id: game.id,
          name: game.name,
          coverImage: game.coverImage,
          headerImage: game.headerImage,
          description: game.description,
          protonRating: game.protonRating,
          protonTier: game.protonTier,
          customProperties: {},
        });
      }
      
      res.json({ 
        success: true, 
        gamesCount: enrichedGames.length,
        message: `Successfully synced ${enrichedGames.length} games from Steam` 
      });
    } catch (error) {
      console.error("Error syncing from Steam:", error);
      res.status(500).json({ 
        error: "Failed to sync from Steam", 
        details: error instanceof Error ? error.message : "Unknown error" 
      });
    }
  });

  // Notion Sync Routes
  app.post("/api/sync/notion", async (req, res) => {
    try {
      const config = await storage.getUserConfig();
      
      if (!config || !config.notionToken || !config.notionDatabaseId) {
        res.status(400).json({ error: "Notion integration token and database ID are required" });
        return;
      }
      
      // Get games from database
      const games = await storage.getGames();
      
      if (games.length === 0) {
        res.status(400).json({ error: "No games to sync. Please sync from Steam first." });
        return;
      }
      
      // Get custom columns
      const customColumns = await storage.getCustomColumns();
      
      // Sync to Notion
      const result = await notionService.syncGames(
        config.notionToken,
        config.notionDatabaseId,
        games,
        customColumns
      );
      
      res.json({
        success: result.success,
        synced: result.synced,
        failed: result.failed,
        errors: result.errors,
        message: `Successfully synced ${result.synced} games to Notion${result.failed > 0 ? `, ${result.failed} failed` : ""}`,
      });
    } catch (error) {
      console.error("Error syncing to Notion:", error);
      res.status(500).json({
        error: "Failed to sync to Notion",
        details: error instanceof Error ? error.message : "Unknown error",
      });
    }
  });

  return httpServer;
}
