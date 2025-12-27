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

  async getGamesWithDetails(apiKey: string, steamId: string) {
    const ownedGames = await this.getOwnedGames(apiKey, steamId);
    
    // Fetch details for each game (with rate limiting)
    const gamesWithDetails = [];
    
    for (const game of ownedGames) {
      const details = await this.getGameDetails(game.appid);
      
      gamesWithDetails.push({
        id: game.appid.toString(),
        name: details?.name || game.name,
        coverImage: details?.coverImage || `https://steamcdn-a.akamaihd.net/steam/apps/${game.appid}/library_600x900.jpg`,
        headerImage: details?.headerImage || `https://steamcdn-a.akamaihd.net/steam/apps/${game.appid}/header.jpg`,
        description: details?.description || "",
        playtime: game.playtime_forever,
      });
      
      // Rate limiting: wait 250ms between requests to avoid Steam API rate limits
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    
    return gamesWithDetails;
  }
}

export const steamService = new SteamService();
