interface SteamGame {
  appid: number;
  name: string;
  playtime_forever: number;
  img_icon_url?: string;
  img_logo_url?: string;
}

interface SteamOwnedGamesResponse {
  response: {
    game_count: number;
    games: SteamGame[];
  };
}

interface SteamAppDetailsResponse {
  [appid: string]: {
    success: boolean;
    data?: {
      name: string;
      short_description: string;
      header_image: string;
      capsule_image: string;
      screenshots?: Array<{
        id: number;
        path_thumbnail: string;
        path_full: string;
      }>;
    };
  };
}

interface SteamReviewsResponse {
  success: number;
  query_summary: {
    num_reviews: number;
    review_score: number;
    review_score_desc: string;
    total_positive: number;
    total_negative: number;
    total_reviews: number;
  };
}

export class SteamService {
  private readonly baseUrl = "https://api.steampowered.com";
  private readonly storeApiUrl = "https://store.steampowered.com/api";

  async getOwnedGames(apiKey: string, steamId: string): Promise<SteamGame[]> {
    const url = `${this.baseUrl}/IPlayerService/GetOwnedGames/v0001/?key=${apiKey}&steamid=${steamId}&include_appinfo=1&include_played_free_games=1&format=json`;
    
    try {
      const response = await fetch(url);
      
      if (!response.ok) {
        throw new Error(`Steam API error: ${response.status} ${response.statusText}`);
      }
      
      const data: SteamOwnedGamesResponse = await response.json();
      return data.response.games || [];
    } catch (error) {
      console.error("Error fetching owned games from Steam:", error);
      throw error;
    }
  }

  async getGameDetails(appId: number): Promise<{
    name: string;
    description: string;
    headerImage: string;
    coverImage: string;
  } | null> {
    const url = `${this.storeApiUrl}/appdetails?appids=${appId}`;
    
    try {
      const response = await fetch(url);
      
      if (!response.ok) {
        console.warn(`Failed to fetch details for app ${appId}: ${response.status}`);
        return null;
      }
      
      const data: SteamAppDetailsResponse = await response.json();
      const appData = data[appId.toString()];
      
      if (!appData || !appData.success || !appData.data) {
        return null;
      }
      
      return {
        name: appData.data.name,
        description: appData.data.short_description,
        headerImage: appData.data.header_image,
        coverImage: appData.data.capsule_image,
      };
    } catch (error) {
      console.error(`Error fetching details for app ${appId}:`, error);
      return null;
    }
  }

  async getGameReviews(appId: number): Promise<{
    rating: string;
    ratingTotal: number;
    ratingPositivePct: number;
  } | null> {
    const url = `${this.storeApiUrl}/appreviews/${appId}?json=1&language=all&purchase_type=all`;
    
    try {
      const response = await fetch(url);
      
      if (!response.ok) {
        console.warn(`Failed to fetch reviews for app ${appId}: ${response.status}`);
        return null;
      }
      
      const data: SteamReviewsResponse = await response.json();
      
      if (!data.success || !data.query_summary) {
        return null;
      }
      
      const summary = data.query_summary;
      const total = summary.total_reviews;
      const positivePct = total > 0 ? Math.round((summary.total_positive / total) * 100) : 0;
      
      // Map Steam's review_score_desc to our normalized format
      const rating = this.normalizeRating(summary.review_score_desc, positivePct);
      
      return {
        rating,
        ratingTotal: total,
        ratingPositivePct: positivePct,
      };
    } catch (error) {
      console.error(`Error fetching reviews for app ${appId}:`, error);
      return null;
    }
  }

  private normalizeRating(steamDesc: string, positivePct: number): string {
    const desc = steamDesc.toLowerCase();
    
    if (desc.includes("overwhelmingly positive")) return "09 overwhelmingly positive 😍";
    if (desc.includes("very positive")) return "08 very positive 🙂";
    if (desc === "positive") return "07 positive 👍";
    if (desc.includes("mostly positive")) return "06 mostly positive 🙂‍↕️";
    if (desc.includes("mixed")) return "05 mixed 😐";
    if (desc.includes("mostly negative")) return "04 mostly negative 👎";
    if (desc === "negative") return "03 negative 😕";
    if (desc.includes("very negative")) return "02 very negative 😬";
    if (desc.includes("overwhelmingly negative")) return "01 overwhelmingly negative 💣";
    
    // Fallback based on percentage
    if (positivePct === 0) return "00 no reviews 0️⃣";
    if (positivePct >= 95) return "09 overwhelmingly positive 😍";
    if (positivePct >= 80) return "08 very positive 🙂";
    if (positivePct >= 70) return "07 positive 👍";
    if (positivePct >= 60) return "06 mostly positive 🙂‍↕️";
    if (positivePct >= 40) return "05 mixed 😐";
    if (positivePct >= 30) return "04 mostly negative 👎";
    if (positivePct >= 20) return "03 negative 😕";
    if (positivePct >= 10) return "02 very negative 😬";
    return "01 overwhelmingly negative 💣";
  }

  async getGamesWithDetails(apiKey: string, steamId: string) {
    const ownedGames = await this.getOwnedGames(apiKey, steamId);
    
    const gamesWithDetails = [];
    
    for (const game of ownedGames) {
      const [details, reviews] = await Promise.all([
        this.getGameDetails(game.appid),
        this.getGameReviews(game.appid),
      ]);
      
      gamesWithDetails.push({
        id: game.appid.toString(),
        name: details?.name || game.name,
        coverImage: details?.coverImage || `https://steamcdn-a.akamaihd.net/steam/apps/${game.appid}/library_600x900.jpg`,
        headerImage: details?.headerImage || `https://steamcdn-a.akamaihd.net/steam/apps/${game.appid}/header.jpg`,
        description: details?.description || "",
        playtime: game.playtime_forever,
        steamRating: reviews?.rating || "00 no reviews 0️⃣",
        ratingTotal: reviews?.ratingTotal || 0,
        ratingPositivePct: reviews?.ratingPositivePct || 0,
      });
      
      // Rate limiting: wait 300ms between requests to avoid Steam API rate limits
      await new Promise(resolve => setTimeout(resolve, 300));
    }
    
    return gamesWithDetails;
  }
}

export const steamService = new SteamService();
