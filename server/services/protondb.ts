interface ProtonDBReport {
  appId: number;
  rating: string;
  tier: string;
}

interface ProtonDBResponse {
  bestReportedTier: string;
  confidence: string;
  score: number;
  tier: string;
  total: number;
  trendingTier: string;
}

export class ProtonDBService {
  private readonly baseUrl = "https://www.protondb.com/api/v1";

  async getGameRating(appId: string): Promise<{ rating: string; tier: string } | null> {
    const url = `${this.baseUrl}/reports/summaries/${appId}.json`;
    
    try {
      const response = await fetch(url);
      
      if (!response.ok) {
        // Game not found in ProtonDB or API error
        return null;
      }
      
      const data: ProtonDBResponse = await response.json();
      
      return {
        rating: data.tier || "unknown",
        tier: data.bestReportedTier || data.tier || "unknown",
      };
    } catch (error) {
      console.error(`Error fetching ProtonDB rating for app ${appId}:`, error);
      return null;
    }
  }

  async enrichGamesWithProtonData<T extends { id: string }>(games: T[]): Promise<(T & { protonRating: string | null; protonTier: string | null })[]> {
    const enrichedGames: (T & { protonRating: string | null; protonTier: string | null })[] = [];
    
    for (const game of games) {
      const protonData = await this.getGameRating(game.id);
      
      enrichedGames.push({
        ...game,
        protonRating: protonData?.rating || null,
        protonTier: protonData?.tier || null,
      });
      
      // Rate limiting: wait 200ms between requests
      await new Promise(resolve => setTimeout(resolve, 200));
    }
    
    return enrichedGames;
  }

  getTierColor(tier: string): string {
    const colors: { [key: string]: string } = {
      platinum: "#b4c7dc",
      gold: "#daa520",
      silver: "#c0c0c0",
      bronze: "#cd7f32",
      borked: "#ff0000",
      unknown: "#6b7280",
    };
    
    return colors[tier.toLowerCase()] || colors.unknown;
  }
}

export const protondbService = new ProtonDBService();
