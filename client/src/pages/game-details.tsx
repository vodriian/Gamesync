import { useRoute } from "wouter";
import { useQuery } from "@tanstack/react-query";
import { GameData } from "@/lib/types";
import { getGame, gameToGameData } from "@/lib/api-service";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ArrowLeft, Clock, ExternalLink, Share2, Star, ThumbsUp, Database, Settings, ScrollText } from "lucide-react";
import { Link } from "wouter";
import { cn } from "@/lib/utils";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { SyncLog } from "@/components/sync-log";
import { useState } from "react";


export default function GameDetails() {
  const [match, params] = useRoute("/game/:appid");
  const appid = params?.appid ? parseInt(params.appid) : 0;
  const [openLogs, setOpenLogs] = useState(false);
  const [openSettings, setOpenSettings] = useState(false);
  const [logs] = useState([]);

  const { data: game, isLoading } = useQuery({
    queryKey: ['game', appid],
    queryFn: async () => {
      try {
        const backendGame = await getGame(appid.toString());
        return gameToGameData(backendGame);
      } catch (error) {
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
    <div className="h-screen overflow-hidden bg-background text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Header */}
      <header className="border-b border-border/40 bg-background/80 backdrop-blur-md sticky top-0 z-50">
        <div className="container mx-auto px-4 md:px-6 h-16 flex items-center justify-between gap-4">
          <Link href="/">
            <div className="flex items-center gap-2 md:gap-3 shrink-0 cursor-pointer">
              <div className="w-7 h-7 md:w-8 md:h-8 rounded bg-primary/20 border border-primary/50 flex items-center justify-center">
                <Database className="w-3.5 h-3.5 md:w-4 md:h-4 text-primary animate-pulse" />
              </div>
              <h1 className="text-lg md:text-2xl font-display font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white to-white/60">
                GAMESYNC <span className="text-primary text-[10px] md:text-sm align-top">PRO</span>
              </h1>
            </div>
          </Link>

          <div className="flex items-center gap-2 md:gap-3">
            {/* System Icons */}
            <div className="flex items-center gap-1 shrink-0">
              <Dialog open={openLogs} onOpenChange={setOpenLogs}>
                <DialogTrigger asChild>
                  <Button variant="ghost" size="icon" className="text-muted-foreground hover:text-foreground h-9 w-9">
                    <ScrollText className="w-4 h-4" />
                  </Button>
                </DialogTrigger>
                <DialogContent className="sm:max-w-[600px] border-border/50 bg-card/95 backdrop-blur-xl max-h-[80vh] flex flex-col">
                  <DialogHeader>
                    <div className="flex items-center justify-between pr-8">
                      <DialogTitle className="font-display tracking-wide text-xl">Operations Log</DialogTitle>
                      <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-muted/30 border border-border/50 text-xs font-mono text-muted-foreground">
                         <div className="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.5)]" />
                         SYSTEM ONLINE
                      </div>
                    </div>
                    <DialogDescription>
                      Real-time synchronization activity and system events.
                    </DialogDescription>
                  </DialogHeader>
                  <div className="flex-1 overflow-hidden min-h-[300px] flex flex-col gap-4">
                    <SyncLog logs={logs} className="flex-1 border border-border/50 rounded-md" />
                  </div>
                </DialogContent>
              </Dialog>

              <Dialog open={openSettings} onOpenChange={setOpenSettings}>
                <DialogTrigger asChild>
                  <Button variant="ghost" size="icon" className="text-muted-foreground hover:text-foreground h-9 w-9">
                    <Settings className="w-4 h-4" />
                  </Button>
                </DialogTrigger>
                <DialogContent className="sm:max-w-[500px] border-border/50 bg-card/95 backdrop-blur-xl">
                  <DialogHeader>
                    <DialogTitle className="font-display tracking-wide text-xl">Configuration</DialogTitle>
                    <DialogDescription>
                      Enter your API keys to enable live synchronization.
                    </DialogDescription>
                  </DialogHeader>
                  <div className="text-sm text-muted-foreground p-4 text-center">
                    Settings configuration will be available here.
                  </div>
                </DialogContent>
              </Dialog>
            </div>
          </div>
        </div>
      </header>

      {/* Sticky Action Bar */}
      <div className="shrink-0 sticky top-16 z-40 border-b border-border/30 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container mx-auto px-4 md:px-6 py-2 flex items-center justify-between gap-4">
          <div className="flex-1 flex justify-start">
            <Link href="/">
              <Button variant="ghost" size="sm" className="text-muted-foreground hover:text-foreground gap-2 -ml-2">
                <ArrowLeft className="w-4 h-4" />
                <span className="hidden sm:inline">Back to Library</span>
                <span className="sm:hidden">Back</span>
              </Button>
            </Link>
          </div>
          
          <div className="flex-1 flex justify-center min-w-0">
             <span className="font-display font-bold text-sm md:text-base truncate">
               {game.name}
             </span>
          </div>

          <div className="flex-1 flex justify-end gap-2">
            <Button size="sm" className="bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm gap-2 h-8">
              <Share2 className="w-3.5 h-3.5" />
              <span className="hidden sm:inline">Share</span>
            </Button>
            <Button size="sm" variant="outline" className="bg-background/50 border-border/50 hover:bg-muted gap-2 h-8" asChild>
              <a href={game.store_url} target="_blank" rel="noreferrer">
                <ExternalLink className="w-3.5 h-3.5" />
                <span className="hidden sm:inline">Store</span>
              </a>
            </Button>
          </div>
        </div>
      </div>

      {/* Scrollable Content */}
      <div className="flex-1 overflow-y-auto">
        {/* Hero Cover Image - Notion Style Full Width */}
        <div className="relative w-full h-[24vh] min-h-[160px] max-h-[320px] overflow-hidden bg-muted">
          {game.header_url ? (
            <img 
              src={game.header_url} 
              alt={game.name} 
              className="w-full h-full object-cover opacity-90"
            />
          ) : (
            <div className="w-full h-full bg-gradient-to-br from-primary/20 to-purple-500/20" />
          )}
          <div className="absolute inset-0 bg-gradient-to-t from-background/40 to-transparent" />
        </div>

        {/* Game Info Section */}
        <div className="container mx-auto px-4 md:px-6 relative">
          
          {/* Floating Icon/Poster - Overlapping the banner */}
          <div className="-mt-12 md:-mt-16 mb-6 relative z-10 w-32 md:w-48 aspect-[460/215] rounded-lg shadow-2xl overflow-hidden border-4 border-background bg-background mx-4 md:mx-0 shadow-black/40">
            <img 
              src={game.header_url || game.cover_url} 
              alt={game.name} 
              className="w-full h-full object-cover"
            />
          </div>

          <div className="pb-10">
            {/* Badges */}
            <div className="flex items-center gap-3 mb-4">
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

            {/* Title */}
            <h1 className="text-3xl md:text-5xl lg:text-6xl font-display font-bold text-foreground tracking-tight leading-tight mb-4">
              {game.name}
            </h1>

            {/* Stats Row */}
            <div className="flex flex-wrap items-center gap-4 md:gap-6 text-muted-foreground font-medium mb-6">
              <div className="flex items-center gap-2">
                <Clock className="w-5 h-5 text-primary" />
                <span className="font-mono text-base md:text-lg">{Math.round(game.playtime_forever / 60)}h Played</span>
              </div>
              {game.proton?.score && (
                <div className="flex items-center gap-2">
                  <ThumbsUp className="w-5 h-5 text-green-500" />
                  <span className="font-mono text-base md:text-lg">{game.proton.score}% Positive</span>
                </div>
              )}
            </div>

            {/* Action Buttons - Moved to sticky header */}
            <div className="hidden flex-col sm:flex-row gap-3 mb-8">
              <Button size="lg" className="bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg shadow-primary/20 gap-2 flex-1 sm:flex-none">
                <Share2 className="w-4 h-4" />
                Share
              </Button>
              <Button size="lg" variant="outline" className="bg-card/50 border-border/50 hover:bg-card gap-2 flex-1 sm:flex-none" asChild>
                <a href={game.store_url} target="_blank" rel="noreferrer">
                  <ExternalLink className="w-4 h-4" />
                  Steam Store
                </a>
              </Button>
            </div>

            <Separator className="bg-border/30 my-6" />

            {/* Content Grid */}
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
              {/* Main Content: About */}
              <div className="lg:col-span-2 space-y-8">
                <div className="space-y-4">
                  <h2 className="text-xl font-display font-bold text-foreground/90">About This Game</h2>
                  <p className="text-muted-foreground leading-relaxed text-base whitespace-pre-wrap">
                    {game.description || "No description available for this game."}
                  </p>
                </div>
              </div>

              {/* Sidebar: Properties */}
              <div className="space-y-6">
                <div className="rounded-xl border border-border/50 bg-card/30 backdrop-blur-sm p-6 space-y-6">
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
                      <span className="text-muted-foreground">Rating</span>
                      <div className="text-right">
                        <Badge variant="outline" className="text-xs border-yellow-500/30 text-yellow-400 bg-yellow-500/10">
                          {game.customProperties?.rating || "00 no reviews 0️⃣"}
                        </Badge>
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-sm">
                      <span className="text-muted-foreground">Tier</span>
                      <div className="text-right">
                        <Badge className={cn("text-xs", TIER_COLORS[tier])}>
                          {game.customProperties?.tier || `${tier}`}
                        </Badge>
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-sm">
                      <span className="text-muted-foreground">Confidence</span>
                      <span className="text-right text-xs">
                        {game.customProperties?.confidence || "00 unknown ❓"}
                      </span>
                    </div>
                    
                    <div className="grid grid-cols-2 gap-2 text-sm">
                      <span className="text-muted-foreground">Playtime</span>
                      <span className="text-right font-mono">{Math.round(game.playtime_forever / 60)}h</span>
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
        </div>
      </div>
    </div>
  );
}
