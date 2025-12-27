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

  private getHeaders(token: string) {
    return {
      "Authorization": `Bearer ${token}`,
      "Notion-Version": this.notionVersion,
      "Content-Type": "application/json",
    };
  }

  private convertPropertyValue(value: any, columnType: string): NotionProperty {
    switch (columnType) {
      case "text":
        return {
          rich_text: [
            {
              text: { content: value?.toString() || "" },
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
              text: { content: value?.toString() || "" },
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
      description?: string | null;
      protonTier?: string | null;
      protonConfidence?: string | null;
      steamRating?: string | null;
      ratingTotal?: number | null;
      ratingPositivePct?: number | null;
      status?: string | null;
      customProperties?: any;
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

      // Add system properties
      if (game.protonTier) {
        properties["Tier"] = this.convertPropertyValue(game.protonTier, "select");
      }
      if (game.protonConfidence) {
        properties["Confidence"] = this.convertPropertyValue(game.protonConfidence, "select");
      }
      if (game.steamRating) {
        properties["Rating"] = this.convertPropertyValue(game.steamRating, "select");
      }
      if (game.ratingTotal !== undefined && game.ratingTotal !== null) {
        properties["RatingTotal"] = this.convertPropertyValue(game.ratingTotal, "number");
      }
      if (game.ratingPositivePct !== undefined && game.ratingPositivePct !== null) {
        properties["RatingPositivePct"] = this.convertPropertyValue(game.ratingPositivePct, "number");
      }
      if (game.status) {
        properties["Status"] = this.convertPropertyValue(game.status, "select");
      }

      // Add Steam Store URL
      properties["SteamURL"] = this.convertPropertyValue(
        `https://store.steampowered.com/app/${game.id}/`,
        "url"
      );

      // Add cover image if available
      let coverConfig: any = undefined;
      if (game.coverImage) {
        coverConfig = {
          external: { url: game.coverImage },
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

      // Add description as page content if available
      if (game.description) {
        pageData.children = [
          {
            object: "block",
            type: "heading_2",
            heading_2: {
              rich_text: [{ text: { content: "About this game" } }],
            },
          },
          {
            object: "block",
            type: "paragraph",
            paragraph: {
              rich_text: [{ text: { content: game.description } }],
            },
          },
        ];
      }

      const response = await fetch(`${this.baseUrl}/pages`, {
        method: "POST",
        headers: this.getHeaders(token),
        body: JSON.stringify(pageData),
      });

      if (!response.ok) {
        const errorData = await response.json();
        console.error("Notion API error:", errorData);
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
      description?: string | null;
      protonTier?: string | null;
      protonConfidence?: string | null;
      steamRating?: string | null;
      ratingTotal?: number | null;
      ratingPositivePct?: number | null;
      status?: string | null;
      customProperties?: any;
    }>,
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; synced: number; failed: number; errors: string[] }> {
    let synced = 0;
    let failed = 0;
    const errors: string[] = [];

    for (const game of games) {
      const result = await this.createPage(token, databaseId, game, customColumns);
      
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
