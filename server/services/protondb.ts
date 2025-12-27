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

  async getGameRating(appId: string): Promise<{ 
    tier: string; 
    confidence: string;
  } | null> {
    const url = `${this.baseUrl}/reports/summaries/${appId}.json`;
    
    try {
      const response = await fetch(url);
      
      if (!response.ok) {
        return null;
      }
      
      const data: ProtonDBResponse = await response.json();
      
      return {
        tier: this.normalizeTier(data.tier || data.bestReportedTier),
        confidence: this.normalizeConfidence(data.confidence, data.total),
      };
    } catch (error) {
      console.error(`Error fetching ProtonDB rating for app ${appId}:`, error);
      return null;
    }
  }

  private normalizeTier(tier: string): string {
    const t = (tier || "").toLowerCase();
    
    if (t === "native") return "06 native 🌿";
    if (t === "platinum") return "05 platinum 👑";
    if (t === "gold") return "04 gold 🥇";
    if (t === "silver") return "03 silver 🥈";
    if (t === "bronze") return "02 bronze 🥉";
    if (t === "borked") return "01 borked 💀";
    if (t === "pending") return "00 pending ⏳";
    
    return "00 unknown ❓";
  }

  private normalizeConfidence(confidence: string, reportCount: number): string {
    const c = (confidence || "").toLowerCase();
    
    // ProtonDB uses "good", "adequate", "inadequate" or similar
    if (c === "good" || c === "high" || reportCount >= 50) return "03 high ✅";
    if (c === "adequate" || c === "medium" || reportCount >= 10) return "02 medium 🟡";
    if (c === "inadequate" || c === "low" || reportCount >= 1) return "01 low ⚠️";
    
    return "00 unknown ❓";
  }

  async enrichGamesWithProtonData<T extends { id: string }>(games: T[]): Promise<(T & { 
    protonTier: string; 
    protonConfidence: string;
  })[]> {
    const enrichedGames: (T & { protonTier: string; protonConfidence: string })[] = [];
    
    for (const game of games) {
      const protonData = await this.getGameRating(game.id);
      
      enrichedGames.push({
        ...game,
        protonTier: protonData?.tier || "00 unknown ❓",
        protonConfidence: protonData?.confidence || "00 unknown ❓",
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
      native: "#22c55e",
      unknown: "#6b7280",
    };
    
    // Extract tier name from formatted string (e.g., "05 platinum 👑" -> "platinum")
    const tierName = tier.split(" ")[1] || tier.toLowerCase();
    return colors[tierName] || colors.unknown;
  }
}

export const protondbService = new ProtonDBService();
