import type { CustomColumn } from "@shared/schema";

export class CraftService {
  private parseApiUrl(apiUrl: string): { baseUrl: string; token?: string } {
    // Extract token from URL if present (format: https://connect.craft.do/links/TOKEN)
    const tokenMatch = apiUrl.match(/links\/([A-Za-z0-9_-]+)/);
    const token = tokenMatch ? tokenMatch[1] : undefined;
    
    // Extract base URL
    const baseUrl = apiUrl.split('/links/')[0];
    
    return { baseUrl, token };
  }

  async createDocument(
    apiUrl: string,
    collectionId: string,
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
      customProperties?: any;
    },
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; documentId?: string; error?: string }> {
    try {
      const { baseUrl, token } = this.parseApiUrl(apiUrl);
      
      if (!token) {
        return {
          success: false,
          error: "Invalid API URL format. Expected format: https://connect.craft.do/links/TOKEN",
        };
      }

      // Build document content
      let content = `# ${game.name}\n\n`;
      
      // Add cover image if available
      if (game.coverImage) {
        content += `![${game.name}](${game.coverImage})\n\n`;
      }
      
      // Add description
      if (game.description) {
        content += `## About this game\n\n${game.description}\n\n`;
      }
      
      // Add system properties
      content += `## Game Details\n\n`;
      content += `**Steam URL**: https://store.steampowered.com/app/${game.id}/\n`;
      if (game.protonTier) {
        content += `**Tier**: ${game.protonTier}\n`;
      }
      if (game.protonConfidence) {
        content += `**Confidence**: ${game.protonConfidence}\n`;
      }
      if (game.steamRating) {
        content += `**Rating**: ${game.steamRating}\n`;
      }
      if (game.ratingTotal !== undefined && game.ratingTotal !== null) {
        content += `**Total Reviews**: ${game.ratingTotal}\n`;
      }
      if (game.ratingPositivePct !== undefined && game.ratingPositivePct !== null) {
        content += `**Positive %**: ${game.ratingPositivePct}%\n`;
      }
      content += `\n`;
      
      // Add custom properties (non-system columns)
      const nonSystemColumns = customColumns.filter(col => col.isSystem !== 1);
      if (game.customProperties && nonSystemColumns.length > 0) {
        content += `## Custom Properties\n\n`;
        for (const column of nonSystemColumns) {
          const value = game.customProperties[column.name];
          if (value !== undefined && value !== null) {
            content += `**${column.name}**: ${value}\n`;
          }
        }
      }

      // Craft API typically expects markdown content
      const payload = {
        spaceId: collectionId,
        content: content,
        title: game.name,
      };

      const response = await fetch(`${baseUrl}/links/${token}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error("Craft API error:", errorText);
        return {
          success: false,
          error: `HTTP ${response.status}: ${errorText}`,
        };
      }

      const result = await response.json();
      return {
        success: true,
        documentId: result.id || result.documentId,
      };
    } catch (error) {
      console.error("Error creating Craft document:", error);
      return {
        success: false,
        error: error instanceof Error ? error.message : "Unknown error",
      };
    }
  }

  async syncGames(
    apiUrl: string,
    collectionId: string,
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
      customProperties?: any;
    }>,
    customColumns: CustomColumn[]
  ): Promise<{ success: boolean; synced: number; failed: number; errors: string[] }> {
    let synced = 0;
    let failed = 0;
    const errors: string[] = [];

    for (const game of games) {
      const result = await this.createDocument(apiUrl, collectionId, game, customColumns);
      
      if (result.success) {
        synced++;
      } else {
        failed++;
        errors.push(`${game.name}: ${result.error}`);
      }

      // Rate limiting: wait 500ms between requests
      await new Promise(resolve => setTimeout(resolve, 500));
    }

    return {
      success: failed === 0,
      synced,
      failed,
      errors,
    };
  }
}

export const craftService = new CraftService();
