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
      detailed_description: string;
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

  /**
   * Strips HTML tags and converts to readable plain text
   */
  private stripHtml(html: string): string {
    if (!html) return "";
    
    return html
      // Remove video and source tags completely
      .replace(/<video[^>]*>[\s\S]*?<\/video>/gi, "")
      .replace(/<source[^>]*\/?>/gi, "")
      // Remove img tags but keep alt text if present
      .replace(/<img[^>]*alt=["']([^"']*)["'][^>]*\/?>/gi, "$1")
      .replace(/<img[^>]*\/?>/gi, "")
      // Remove span containers for media
      .replace(/<span class="bb_img_ctn"[^>]*>[\s\S]*?<\/span>/gi, "")
      // Convert br tags to newlines
      .replace(/<br\s*\/?>/gi, "\n")
      // Convert paragraph and div closings to double newlines
      .replace(/<\/(p|div|h[1-6])>/gi, "\n\n")
      // Convert list items to bullet points
      .replace(/<li[^>]*>/gi, "• ")
      .replace(/<\/li>/gi, "\n")
      // Remove all remaining HTML tags
      .replace(/<[^>]+>/g, "")
      // Decode common HTML entities
      .replace(/&nbsp;/gi, " ")
      .replace(/&amp;/gi, "&")
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">")
      .replace(/&quot;/gi, '"')
      .replace(/&#39;/gi, "'")
      .replace(/&apos;/gi, "'")
      // Clean up excessive whitespace
      .replace(/\n{3,}/g, "\n\n")
      .replace(/[ \t]+/g, " ")
      .trim();
  }

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
      const response = await fetch(url, {
        headers: {
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        }
      });
      
      if (!response.ok) {
        if (response.status === 429) {
          console.warn(`[DETAILS] Rate limited for app ${appId}`);
          return null;
        }
        console.warn(`Failed to fetch details for app ${appId}: ${response.status}`);
        return null;
      }
      
      const data: SteamAppDetailsResponse = await response.json();
      const appData = data[appId.toString()];
      
      if (!appData || !appData.success || !appData.data) {
        return null;
      }
      
      const rawDescription = appData.data.detailed_description || appData.data.short_description;
      
      return {
        name: appData.data.name,
        description: this.stripHtml(rawDescription),
        headerImage: appData.data.header_image,
        coverImage: appData.data.capsule_image,
      };
    } catch (error) {
      console.error(`Error fetching details for app ${appId}:`, error);
      return null;
    }
  }

  async getGameReviews(appId: number, retryCount = 0): Promise<{
    rating: string;
    ratingTotal: number;
    ratingPositivePct: number;
  } | null> {
    const MAX_RETRIES = 3;
    const url = `${this.storeApiUrl}/appreviews/${appId}?json=1&language=all&purchase_type=all`;
    
    try {
      const response = await fetch(url, {
        headers: {
          'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
          'Accept': 'application/json',
          'Accept-Language': 'en-US,en;q=0.9',
          'Referer': `https://store.steampowered.com/app/${appId}/`
        }
      });
      
      if (response.status === 403) {
        console.warn(`[REVIEWS] API forbidden (403) for app ${appId}, switching to HTML fallback immediately`);
        return this.fetchReviewsFromHtml(appId);
      }

      if (response.status === 429) {
        if (retryCount < MAX_RETRIES) {
          // Wait longer with each retry: 2s, 4s, 8s
          const waitTime = Math.pow(2, retryCount + 1) * 1000;
          console.log(`[REVIEWS] Rate limited (${response.status}) for ${appId}, waiting ${waitTime}ms before retry ${retryCount + 1}/${MAX_RETRIES}`);
          await new Promise(resolve => setTimeout(resolve, waitTime));
          return this.getGameReviews(appId, retryCount + 1);
        }
        
        console.warn(`[REVIEWS] Max retries reached for app ${appId}, trying HTML fallback...`);
        // Fallback to HTML scraping if API fails
        return this.fetchReviewsFromHtml(appId);
      }
      
      if (!response.ok) {
        console.warn(`[REVIEWS] API error for app ${appId}: ${response.status}`);
        return null;
      }
      
      const data: SteamReviewsResponse = await response.json();
      
      if (data.success !== 1 || !data.query_summary) {
        return null;
      }
      
      const summary = data.query_summary;
      const total = summary.total_reviews;
      const positivePct = total > 0 ? Math.round((summary.total_positive / total) * 100) : 0;
      
      // Map Steam's review_score_desc to our normalized format
      const rating = this.normalizeRating(summary.review_score_desc, positivePct);
      
      console.log(`[REVIEWS] ${appId}: ${summary.review_score_desc} (${total} reviews)`);
      
      return {
        rating,
        ratingTotal: total,
        ratingPositivePct: positivePct,
      };
    } catch (error) {
      console.error(`[REVIEWS] Error for app ${appId}:`, error);
      return null;
    }
  }

  // Helper method to fetch HTML page and parse review data as a last resort fallback
  // This bypasses the API rate limits by just acting like a browser visiting the page
  private async fetchReviewsFromHtml(appId: number): Promise<{
    rating: string;
    ratingTotal: number;
    ratingPositivePct: number;
  } | null> {
     try {
       // We use a different URL structure to look like a normal page visit
       const url = `https://store.steampowered.com/app/${appId}/`;
       const response = await fetch(url, {
         headers: {
           'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
           'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
           'Accept-Language': 'en-US,en;q=0.9',
         }
       });

       if (!response.ok) return null;
       const html = await response.text();

       // Extract using regex from the raw HTML
       // Looking for: <span class="game_review_summary positive">Very Positive</span>
       const reviewSummaryMatch = html.match(/class="game_review_summary[^"]*">([^<]+)<\/span>/);
       // Looking for: <meta itemprop="reviewCount" content="12345">
       const totalMatch = html.match(/<meta itemprop="reviewCount" content="(\d+)">/);
       // Looking for: <meta itemprop="ratingValue" content="9"> (this is 0-10 usually)
       // Or looking for the percentage in the tooltip: "85% of the 1,234 user reviews..."
       const percentMatch = html.match(/(\d+)% of the/);

       if (!reviewSummaryMatch) return null;

       const reviewDesc = reviewSummaryMatch[1].trim();
       const total = totalMatch ? parseInt(totalMatch[1]) : 0;
       const percent = percentMatch ? parseInt(percentMatch[1]) : 0;
       
       const rating = this.normalizeRating(reviewDesc, percent);

       console.log(`[REVIEWS-HTML] ${appId}: ${reviewDesc} (${total} reviews)`);
       
       return {
         rating,
         ratingTotal: total,
         ratingPositivePct: percent
       };

     } catch (e) {
       console.error(`[REVIEWS-HTML] Failed for ${appId}`, e);
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
    console.log(`[STEAM SYNC] Starting sync for Steam ID: ${steamId}`);
    const ownedGames = await this.getOwnedGames(apiKey, steamId);
    console.log(`[STEAM SYNC] Found ${ownedGames.length} owned games`);
    
    const gamesWithDetails = [];
    
    // Process games ONE AT A TIME to avoid Steam rate limiting
    // Steam's Reviews API is very sensitive to parallel requests
    for (let i = 0; i < ownedGames.length; i++) {
      const game = ownedGames[i];
      
      // Fetch details (less rate-limited)
      const details = await this.getGameDetails(game.appid);
      
      // Delay between details and reviews to be gentler on the API
      await new Promise(resolve => setTimeout(resolve, 800));
      
      // Fetch reviews separately
      const reviews = await this.getGameReviews(game.appid);
      
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
      
      // Log progress every 10 games
      if ((i + 1) % 10 === 0) {
        console.log(`[STEAM SYNC] Processed ${i + 1}/${ownedGames.length} games`);
      }
      
      // Significant delay between games to avoid global rate limiting (429)
      // Steam allows ~200 requests per 5 minutes per IP on store APIs
      await new Promise(resolve => setTimeout(resolve, 1500));
    }
    
    console.log(`[STEAM SYNC] Completed sync of ${gamesWithDetails.length} games`);
    return gamesWithDetails;
  }
}

export const steamService = new SteamService();
