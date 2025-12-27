import { useRoute } from "wouter";
import { useQuery } from "@tanstack/react-query";
import { GameData } from "@/lib/types";
import { getGame, gameToGameData } from "@/lib/api-service";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ArrowLeft, Clock, ExternalLink, Share2, Star, ThumbsUp } from "lucide-react";
import { Link } from "wouter";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

// Mock "About" text generator since we don't have real Steam API
const MOCK_ABOUT = `
  <h2>About This Game</h2>
  <p>This is a placeholder description for the game. In a real application, this content would be fetched from the Steam Store API's 'appdetails' endpoint.</p>
  <br/>
  <p><strong>Key Features:</strong></p>
  <ul>
    <li>Immersive gameplay mechanics that challenge your skills.</li>
    <li>Stunning visuals and atmospheric sound design.</li>
    <li>A rich, narrative-driven experience with deep lore.</li>
    <li>ProtonDB Verified for excellent performance on Linux and Steam Deck.</li>
  </ul>
  <br/>
  <p>Explore a vast world filled with secrets, engage in intense combat, and uncover the truth behind the mystery. Whether you're a casual player or a hardcore gamer, this title offers something for everyone.</p>
  <br/>
  <p><em>Sync this game to Notion to keep track of your progress, rating, and personal notes!</em></p>
`;

export default function GameDetails() {
  const [match, params] = useRoute("/game/:appid");
  const appid = params?.appid ? parseInt(params.appid) : 0;

  const { data: game, isLoading } = useQuery({
    queryKey: ['game', appid],
    queryFn: async () => {
      try {
        const backendGame = await getGame(appid.toString());
        return gameToGameData(backendGame);
      } catch (error) {
        // Fallback if game not found
        return {
          appid: appid,
          name: "Game Not Found",
          playtime_forever: 0,
          cover_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/header.jpg`,
          store_url: `https://store.steampowered.com/app/${appid}/`,
          proton: { tier: "unknown" },
          notion_status: "pending"
        } as GameData;
      }
    },
    staleTime: Infinity
  });

  if (isLoading || !game) {
    return <div className="min-h-screen bg-background flex items-center justify-center">
      <div className="text-muted-foreground">Loading game details...</div>
    </div>;
  }

  const tier = game.proton?.tier || "unknown";
  
  const TIER_COLORS: Record<string, string> = {
    platinum: "bg-blue-500 text-white border-blue-400",
    gold: "bg-yellow-500 text-white border-yellow-400",
    silver: "bg-gray-400 text-white border-gray-300",
    bronze: "bg-orange-700 text-white border-orange-600",
    borked: "bg-red-600 text-white border-red-500",
    native: "bg-green-600 text-white border-green-500",
    unknown: "bg-slate-700 text-white border-slate-600",
  };

  return (
    <div className="min-h-screen bg-background text-foreground font-sans">
      {/* Hero Header with Blur Effect */}
      <div className="relative h-[400px] w-full overflow-hidden">
        {/* Background Blur */}
        <div 
          className="absolute inset-0 bg-cover bg-center opacity-40 blur-3xl scale-110"
          style={{ backgroundImage: `url(${game.cover_url})` }}
        />
        <div className="absolute inset-0 bg-gradient-to-t from-background via-background/60 to-transparent" />

        <div className="container mx-auto px-6 h-full flex flex-col justify-end pb-8 relative z-10">
          <Link href="/">
            <Button variant="ghost" size="sm" className="absolute top-8 left-6 text-white/70 hover:text-white hover:bg-white/10 gap-2">
              <ArrowLeft className="w-4 h-4" />
              Back to Library
            </Button>
          </Link>

          <div className="flex flex-col md:flex-row gap-8 items-end">
            <div className="shrink-0 rounded-lg overflow-hidden shadow-2xl border border-white/10 w-[300px] aspect-video bg-black">
              <img src={game.cover_url} alt={game.name} className="w-full h-full object-cover" />
            </div>
            
            <div className="flex-1 space-y-4 mb-2">
              <div className="flex items-center gap-3">
                 <Badge className={cn("uppercase tracking-widest text-[10px] py-1 px-2.5 rounded shadow-lg", TIER_COLORS[tier])}>
                   Proton {tier}
                 </Badge>
                 {game.notion_status === 'synced' && (
                   <Badge variant="outline" className="text-green-400 border-green-500/30 bg-green-500/10 gap-1.5">
                     <div className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                     Synced to Notion
                   </Badge>
                 )}
              </div>
              
              <h1 className="text-5xl md:text-7xl font-display font-bold text-white tracking-tight drop-shadow-xl leading-none">
                {game.name}
              </h1>
              
              <div className="flex items-center gap-6 text-white/80 font-medium">
                <div className="flex items-center gap-2">
                  <Clock className="w-5 h-5 text-primary" />
                  <span className="font-mono text-lg">{Math.round(game.playtime_forever / 60)}h Played</span>
                </div>
                {game.proton?.score && (
                   <div className="flex items-center gap-2">
                     <ThumbsUp className="w-5 h-5 text-green-400" />
                     <span className="font-mono text-lg">{game.proton.score}% Positive</span>
                   </div>
                )}
              </div>
            </div>

            <div className="flex gap-3 mb-2">
               <Button size="lg" className="bg-primary hover:bg-primary/90 text-white shadow-lg shadow-primary/20 gap-2">
                 <Share2 className="w-4 h-4" />
                 Share
               </Button>
               <Button size="lg" variant="outline" className="bg-black/20 backdrop-blur border-white/10 hover:bg-white/10 text-white gap-2" asChild>
                 <a href={game.store_url} target="_blank" rel="noreferrer">
                   <ExternalLink className="w-4 h-4" />
                   Steam Store
                 </a>
               </Button>
            </div>
          </div>
        </div>
      </div>

      <div className="container mx-auto px-6 py-12 grid grid-cols-1 lg:grid-cols-3 gap-12">
        {/* Main Content: About */}
        <div className="lg:col-span-2 space-y-8">
           <div className="prose prose-invert prose-lg max-w-none">
             {/* Using dangerouslySetInnerHTML for the mock HTML content */}
             <div dangerouslySetInnerHTML={{ __html: MOCK_ABOUT }} />
           </div>
        </div>

        {/* Sidebar: Properties */}
        <div className="space-y-6">
          <div className="rounded-xl border border-border/50 bg-card/30 backdrop-blur-sm p-6 space-y-6 sticky top-24">
             <div className="flex items-center gap-2 pb-4 border-b border-border/50">
               <Star className="w-5 h-5 text-yellow-500" />
               <h3 className="font-display font-bold text-xl">Properties</h3>
             </div>

             <div className="space-y-4">
               <div className="grid grid-cols-2 gap-2 text-sm">
                 <span className="text-muted-foreground">App ID</span>
                 <span className="font-mono text-right">{game.appid}</span>
               </div>
               
               <div className="grid grid-cols-2 gap-2 text-sm">
                 <span className="text-muted-foreground">Status</span>
                 <div className="text-right">
                   <Badge variant="secondary" className="text-[10px] uppercase">Backlog</Badge>
                 </div>
               </div>

               <div className="grid grid-cols-2 gap-2 text-sm">
                 <span className="text-muted-foreground">Install Size</span>
                 <span className="text-right font-mono">-- GB</span>
               </div>
               
               <div className="grid grid-cols-2 gap-2 text-sm">
                 <span className="text-muted-foreground">Last Played</span>
                 <span className="text-right">Oct 12, 2024</span>
               </div>
             </div>
             
             <Separator className="bg-border/50" />
             
             <div className="space-y-3">
               <h4 className="font-medium text-sm text-muted-foreground uppercase tracking-wider">Custom Tags</h4>
               <div className="flex flex-wrap gap-2">
                 <Badge variant="outline" className="border-primary/30 text-primary bg-primary/5">RPG</Badge>
                 <Badge variant="outline" className="border-primary/30 text-primary bg-primary/5">Open World</Badge>
                 <Badge variant="outline" className="border-primary/30 text-primary bg-primary/5">Story Rich</Badge>
               </div>
             </div>
          </div>
        </div>
      </div>
    </div>
  );
}
