import type { CustomColumn } from "@shared/schema";

interface NotionProperty {
  [key: string]: any;
}

interface NotionPage {
  parent: { database_id: string };
  properties: { [key: string]: NotionProperty };
  children?: any[];
}

export class NotionService {
  private readonly baseUrl = "https://api.notion.com/v1";
  private readonly notionVersion = "2022-06-28";
  private readonly MAX_TEXT_LENGTH = 2000; // Notion's limit for rich_text content

  private getHeaders(token: string) {
    return {
      "Authorization": `Bearer ${token}`,
      "Notion-Version": this.notionVersion,
      "Content-Type": "application/json",
    };
  }

  /**
   * Split text into chunks of max length for Notion's 2000 character limit
   */
  private splitTextIntoChunks(text: string, maxLength: number = this.MAX_TEXT_LENGTH): string[] {
    if (!text || text.length <= maxLength) {
      return text ? [text] : [];
    }

    const chunks: string[] = [];
    let remaining = text;

    while (remaining.length > 0) {
      if (remaining.length <= maxLength) {
        chunks.push(remaining);
        break;
      }

      // Try to split at a sentence boundary
      let splitIndex = remaining.lastIndexOf('. ', maxLength);
      if (splitIndex === -1 || splitIndex < maxLength / 2) {
        // Try to split at a newline
        splitIndex = remaining.lastIndexOf('\n', maxLength);
      }
      if (splitIndex === -1 || splitIndex < maxLength / 2) {
        // Try to split at a space
        splitIndex = remaining.lastIndexOf(' ', maxLength);
      }
      if (splitIndex === -1 || splitIndex < maxLength / 2) {
        // Force split at maxLength
        splitIndex = maxLength;
      }

      chunks.push(remaining.substring(0, splitIndex + 1).trim());
      remaining = remaining.substring(splitIndex + 1).trim();
    }

    return chunks;
  }

  /**
   * Create paragraph blocks from description, handling the 2000 char limit
   */
  private createDescriptionBlocks(description: string): any[] {
    const chunks = this.splitTextIntoChunks(description);
    
    return chunks.map(chunk => ({
      object: "block",
      type: "paragraph",
      paragraph: {
        rich_text: [{ text: { content: chunk } }],
      },
    }));
  }

  /**
   * Check if a page with the given AppID already exists in the database
   */
  private async findPageByAppId(token: string, databaseId: string, appId: string): Promise<string | null> {
    try {
      console.log(`[NOTION DEBUG] Searching for page with AppID: ${appId}`);
      const response = await fetch(`${this.baseUrl}/databases/${databaseId}/query`, {
        method: "POST",
        headers: this.getHeaders(token),
        body: JSON.stringify({
          filter: {
            property: "AppID",
            number: {
              equals: parseInt(appId)
            }
          }
        })
      });

      if (!response.ok) {
        console.error(`[NOTION DEBUG] Search failed with status: ${response.status}`);
        return null;
      }

      const data = await response.json();
      console.log(`[NOTION DEBUG] Found ${data.results?.length || 0} pages for AppID ${appId}`);
      
      if (data.results && data.results.length > 0) {
        return data.results[0].id;
      }
      return null;
    } catch (error) {
      console.error(`Error searching for page with AppID ${appId}:`, error);
      return null;
    }
  }

  /**
   * Update an existing Notion page
   */
  async updatePage(
    token: string,
    pageId: string,
    game: any,
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; error?: string }> {
    try {
      const properties: { [key: string]: NotionProperty } = {};

      // Only update fields that might have changed or are crucial
      
      // Update Playtime
      if (game.playtime !== undefined && game.playtime !== null) {
        properties["Playtime (min)"] = this.convertPropertyValue(game.playtime, "number");
      }

      // Update Tier
      if (game.protonTier) {
        properties["Tier"] = this.convertPropertyValue(game.protonTier, "select");
      }

      // Update Confidence
      if (game.protonConfidence) {
        properties["Confidence"] = this.convertPropertyValue(game.protonConfidence, "select");
      }

      // Update Rating
      if (game.steamRating) {
        properties["Rating"] = this.convertPropertyValue(game.steamRating, "select");
      }

      // Update RatingTotal
      if (game.ratingTotal !== undefined && game.ratingTotal !== null) {
        properties["RatingTotal"] = this.convertPropertyValue(game.ratingTotal, "number");
      }

      // Update RatingPositivePct
      if (game.ratingPositivePct !== undefined && game.ratingPositivePct !== null) {
        properties["RatingPositivePct"] = this.convertPropertyValue(game.ratingPositivePct, "number");
      }
      
      // Update Last sync
      properties["Last sync"] = {
        date: { start: new Date().toISOString() },
      };

      // Update custom properties
      if (game.customProperties && customColumns) {
        for (const column of customColumns) {
          if (column.isSystem === 1) continue;
          const value = game.customProperties[column.name];
          if (value !== undefined && value !== null) {
            properties[column.name] = this.convertPropertyValue(value, column.type);
          }
        }
      }

      const response = await fetch(`${this.baseUrl}/pages/${pageId}`, {
        method: "PATCH",
        headers: this.getHeaders(token),
        body: JSON.stringify({
          properties,
          // We don't update cover/icon/content on every sync to avoid overwriting user customizations
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        console.error("Notion update error:", JSON.stringify(errorData, null, 2));
        return {
          success: false,
          error: errorData.message || `HTTP ${response.status}`,
        };
      }

      return { success: true };
    } catch (error) {
      console.error("Error updating Notion page:", error);
      return {
        success: false,
        error: error instanceof Error ? error.message : "Unknown error",
      };
    }
  }

  private convertPropertyValue(value: any, columnType: string): NotionProperty {
    switch (columnType) {
      case "text":
        return {
          rich_text: [
            {
              text: { content: (value?.toString() || "").substring(0, this.MAX_TEXT_LENGTH) },
            },
          ],
        };
      
      case "number":
        return {
          number: typeof value === "number" ? value : parseFloat(value) || null,
        };
      
      case "date":
        return {
          date: value ? { start: new Date(value).toISOString().split("T")[0] } : null,
        };
      
      case "url":
        return {
          url: value?.toString() || null,
        };
      
      case "select":
        return {
          select: value ? { name: value.toString() } : null,
        };
      
      case "multi-select":
        return {
          multi_select: Array.isArray(value) 
            ? value.map(v => ({ name: v.toString() }))
            : [],
        };
      
      default:
        return {
          rich_text: [
            {
              text: { content: (value?.toString() || "").substring(0, this.MAX_TEXT_LENGTH) },
            },
          ],
        };
    }
  }

  async createPage(
    token: string,
    databaseId: string,
    game: {
      id: string;
      name: string;
      coverImage?: string | null;
      headerImage?: string | null;
      description?: string | null;
      playtime?: number | null;
      protonTier?: string | null;
      protonConfidence?: string | null;
      steamRating?: string | null;
      ratingTotal?: number | null;
      ratingPositivePct?: number | null;
      customProperties?: any;
      lastSynced?: Date | null;
    },
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; pageId?: string; error?: string }> {
    try {
      const properties: { [key: string]: NotionProperty } = {
        Name: {
          title: [
            {
              text: { content: game.name },
            },
          ],
        },
      };

      // Add AppID
      // Try to send as number first, but fallback to rich_text if it fails validation?
      // Actually, we can't retry easily inside the payload. 
      // Assumption: AppID is a Number property. If it's Text, this will fail.
      // Let's support both based on error or just be safe and send as number, which is most logical.
      properties["AppID"] = this.convertPropertyValue(parseInt(game.id), "number");

      // Add Playtime (in minutes)
      if (game.playtime !== undefined && game.playtime !== null) {
        properties["Playtime (min)"] = this.convertPropertyValue(game.playtime, "number");
      } else {
         // Explicitly set to null if undefined to clear it? No, just skip to avoid errors if property doesn't exist
      }

      // Add ProtonDB tier (matching user's Notion property name "Tier")
      if (game.protonTier) {
        properties["Tier"] = this.convertPropertyValue(game.protonTier, "select");
      }

      // Add Confidence
      if (game.protonConfidence) {
        properties["Confidence"] = this.convertPropertyValue(game.protonConfidence, "select");
      }

      // Add Rating
      if (game.steamRating) {
        properties["Rating"] = this.convertPropertyValue(game.steamRating, "select");
      }

      // Add Steam reviews (total count) -> "RatingTotal"
      if (game.ratingTotal !== undefined && game.ratingTotal !== null) {
        properties["RatingTotal"] = this.convertPropertyValue(game.ratingTotal, "number");
      }

      // Add Steam rating % (positive percentage) -> "RatingPositivePct"
      if (game.ratingPositivePct !== undefined && game.ratingPositivePct !== null) {
        properties["RatingPositivePct"] = this.convertPropertyValue(game.ratingPositivePct, "number");
      }

      // Add Steam Store URL -> "SteamURL"
      properties["SteamURL"] = this.convertPropertyValue(
        `https://store.steampowered.com/app/${game.id}/`,
        "url"
      );

      // Add ProtonDB URL (Optional, only if exists in Notion)
      // properties["ProtonDB URL"] = this.convertPropertyValue(
      //   `https://www.protondb.com/app/${game.id}`,
      //   "url"
      // );
      
      // Add Cover URL property if it exists in Notion (or skip if not needed)
      // properties["Cover"] = this.convertPropertyValue(game.coverImage, "url");

      // Add Last sync timestamp
      // Important: Check if "Last sync" is actually a Date property in Notion
      // If it's a Text property, this payload will fail.
      // We'll send it as date, but users should ensure their Notion DB matches.
      properties["Last sync"] = {
        date: { start: new Date().toISOString() },
      };

      // Add cover image URL as a property (for Cover property in Notion)
      // if (game.coverImage) {
      //   properties["Cover"] = this.convertPropertyValue(game.coverImage, "url");
      // }

      // Add cover image if available (as page cover)
      let coverConfig: any = undefined;
      if (game.headerImage || game.coverImage) {
        coverConfig = {
          external: { url: game.headerImage || game.coverImage },
        };
      }

      // Add custom properties based on custom columns (non-system columns)
      if (game.customProperties && customColumns) {
        for (const column of customColumns) {
          if (column.isSystem === 1) continue; // Skip system columns, already handled above
          const value = game.customProperties[column.name];
          if (value !== undefined && value !== null) {
            properties[column.name] = this.convertPropertyValue(value, column.type);
          }
        }
      }

      const pageData: NotionPage = {
        parent: { database_id: databaseId },
        properties,
      };

      // Add cover if available
      if (coverConfig) {
        (pageData as any).cover = coverConfig;
      }

      // Add description as page content if available (split into multiple blocks if needed)
      if (game.description) {
        pageData.children = [
          {
            object: "block",
            type: "heading_2",
            heading_2: {
              rich_text: [{ text: { content: "About this game" } }],
            },
          },
          ...this.createDescriptionBlocks(game.description),
        ];
      }

      const response = await fetch(`${this.baseUrl}/pages`, {
        method: "POST",
        headers: this.getHeaders(token),
        body: JSON.stringify(pageData),
      });

      if (!response.ok) {
        const errorData = await response.json();
        console.error("Notion API error:", JSON.stringify(errorData, null, 2));
        
        // Log the failing payload properties for debugging
        console.error("Failing payload properties:", JSON.stringify(properties, null, 2));

        return {
          success: false,
          error: errorData.message || `HTTP ${response.status}`,
        };
      }

      const result = await response.json();
      return {
        success: true,
        pageId: result.id,
      };
    } catch (error) {
      console.error("Error creating Notion page:", error);
      return {
        success: false,
        error: error instanceof Error ? error.message : "Unknown error",
      };
    }
  }

  async syncGames(
    token: string,
    databaseId: string,
    games: Array<{
      id: string;
      name: string;
      coverImage?: string | null;
      headerImage?: string | null;
      description?: string | null;
      playtime?: number | null;
      protonTier?: string | null;
      protonConfidence?: string | null;
      steamRating?: string | null;
      ratingTotal?: number | null;
      ratingPositivePct?: number | null;
      customProperties?: any;
      lastSynced?: Date | null;
    }>,
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; synced: number; failed: number; errors: string[] }> {
    let synced = 0;
    let failed = 0;
    const errors: string[] = [];

    for (const game of games) {
      // Check if page exists first - CRITICAL: retry once if search fails to be sure
      let existingPageId = await this.findPageByAppId(token, databaseId, game.id);
      
      // Double check if first attempt failed or returned null unexpectedly
      if (!existingPageId) {
         // Brief pause before retry just in case of eventual consistency or rate limit hiccups
         await new Promise(resolve => setTimeout(resolve, 500));
         existingPageId = await this.findPageByAppId(token, databaseId, game.id);
      }
      
      let result;
      if (existingPageId) {
        console.log(`[NOTION SYNC] Updating existing page for ${game.name} (${existingPageId})`);
        // Update existing page
        result = await this.updatePage(token, existingPageId, game, customColumns);
      } else {
        console.log(`[NOTION SYNC] Creating new page for ${game.name}`);
        // Create new page
        result = await this.createPage(token, databaseId, game, customColumns);
      }
      
      if (result.success) {
        synced++;
      } else {
        failed++;
        errors.push(`${game.name}: ${result.error}`);
      }

      // Rate limiting: wait 330ms between requests (Notion rate limit is 3 requests per second)
      await new Promise(resolve => setTimeout(resolve, 330));
    }

    return {
      success: failed === 0,
      synced,
      failed,
      errors,
    };
  }
}

export const notionService = new NotionService();
