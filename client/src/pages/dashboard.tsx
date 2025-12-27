import { useState, useEffect } from "react";
import { AppConfig, GameData, LogEntry, ColumnConfig, DEFAULT_COLUMNS } from "@/lib/types";
import { fetchSteamGames, syncToNotion } from "@/lib/mock-service";
import { GameCard } from "@/components/game-card";
import { GamesTable } from "@/components/games-table";
import { SyncLog } from "@/components/sync-log";
import { ColumnManager } from "@/components/column-manager";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { RefreshCw, Settings, Database, Play, ShieldAlert, LayoutGrid, Table as TableIcon } from "lucide-react";
import { toast } from "@/hooks/use-toast";
import { motion, AnimatePresence } from "framer-motion";
import { cn } from "@/lib/utils";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

export default function Dashboard() {
  const [games, setGames] = useState<GameData[]>([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [viewMode, setViewMode] = useState<"grid" | "table">("grid");
  const [columns, setColumns] = useState<ColumnConfig[]>(DEFAULT_COLUMNS);
  
  const [config, setConfig] = useState<AppConfig>({
    steamKey: "",
    steamId: "",
    notionToken: "",
    notionDbId: ""
  });
  const [openSettings, setOpenSettings] = useState(false);

  // Load initial data
  useEffect(() => {
    loadGames();
  }, []);

  const addLog = (message: string, level: LogEntry["level"] = "info") => {
    const entry: LogEntry = {
      id: Math.random().toString(36).substr(2, 9),
      timestamp: new Date(),
      level,
      message
    };
    setLogs(prev => [...prev, entry]);
  };

  const loadGames = async () => {
    setLoading(true);
    addLog("Fetching library from Steam...", "info");
    try {
      const data = await fetchSteamGames(config.steamKey, config.steamId);
      setGames(data);
      addLog(`Successfully loaded ${data.length} games.`, "success");
    } catch (error) {
      addLog("Failed to fetch games.", "error");
    } finally {
      setLoading(false);
    }
  };

  const handleSync = async () => {
    if (!config.notionToken || !config.notionDbId) {
      toast({
        title: "Configuration Missing",
        description: "Please set your Notion credentials in Settings.",
        variant: "destructive"
      });
      setOpenSettings(true);
      return;
    }

    setSyncing(true);
    
    // Pass columns to sync function in future so it knows what properties to create in Notion
    await syncToNotion(games, config.notionToken, config.notionDbId, (msg, level) => {
      addLog(msg, level);
    });

    // Update local state to show "synced" status for demo purposes
    setGames(prev => prev.map(g => ({ ...g, notion_status: "synced" })));
    setSyncing(false);
  };

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Header */}
      <header className="border-b border-border/40 bg-background/80 backdrop-blur-md sticky top-0 z-50">
        <div className="container mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded bg-primary/20 border border-primary/50 flex items-center justify-center">
              <Database className="w-4 h-4 text-primary animate-pulse" />
            </div>
            <h1 className="text-2xl font-display font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white to-white/60">
              GAMESYNC <span className="text-primary text-sm align-top">PRO</span>
            </h1>
          </div>

          <div className="flex items-center gap-4">
            <div className="hidden md:flex items-center gap-2 px-3 py-1 rounded-full bg-muted/30 border border-border/50 text-xs font-mono text-muted-foreground">
               <div className="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.5)]" />
               SYSTEM ONLINE
            </div>

            <Dialog open={openSettings} onOpenChange={setOpenSettings}>
              <DialogTrigger asChild>
                <Button variant="outline" size="sm" className="gap-2 border-border/50 hover:bg-muted/50">
                  <Settings className="w-4 h-4" />
                  Settings
                </Button>
              </DialogTrigger>
              <DialogContent className="sm:max-w-[500px] border-border/50 bg-card/95 backdrop-blur-xl">
                <DialogHeader>
                  <DialogTitle className="font-display tracking-wide text-xl">Configuration</DialogTitle>
                  <DialogDescription>
                    Enter your API keys to enable live synchronization.
                  </DialogDescription>
                </DialogHeader>
                <div className="grid gap-6 py-4">
                  <div className="space-y-2">
                    <h4 className="font-medium text-primary text-sm uppercase tracking-wider">Steam API</h4>
                    <Separator className="opacity-50" />
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="steamKey" className="text-right text-xs">API Key</Label>
                      <Input 
                        id="steamKey" 
                        type="password"
                        value={config.steamKey} 
                        onChange={e => setConfig({...config, steamKey: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs" 
                        placeholder="XXXXXXXXXXXXXXXX"
                      />
                    </div>
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="steamId" className="text-right text-xs">SteamID64</Label>
                      <Input 
                        id="steamId" 
                        value={config.steamId} 
                        onChange={e => setConfig({...config, steamId: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs" 
                        placeholder="76561198000000000"
                      />
                    </div>
                  </div>

                  <div className="space-y-2">
                    <h4 className="font-medium text-primary text-sm uppercase tracking-wider">Notion API</h4>
                    <Separator className="opacity-50" />
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="notionToken" className="text-right text-xs">Integration Token</Label>
                      <Input 
                        id="notionToken" 
                        type="password"
                        value={config.notionToken} 
                        onChange={e => setConfig({...config, notionToken: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs" 
                        placeholder="secret_..."
                      />
                    </div>
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="dbId" className="text-right text-xs">Database ID</Label>
                      <Input 
                        id="dbId" 
                        value={config.notionDbId} 
                        onChange={e => setConfig({...config, notionDbId: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs" 
                        placeholder="32 chars..."
                      />
                    </div>
                  </div>
                </div>
                <DialogFooter>
                  <Button onClick={() => setOpenSettings(false)}>Save Changes</Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 container mx-auto px-6 py-8 flex gap-8 h-[calc(100vh-64px)] overflow-hidden">
        
        {/* Left Panel: Game Grid */}
        <div className="flex-1 flex flex-col gap-6 overflow-hidden">
          <div className="flex items-center justify-between shrink-0">
             <div>
               <h2 className="text-3xl font-display font-bold">Library</h2>
               <p className="text-muted-foreground text-sm">Manage your Steam collection sync status.</p>
             </div>
             
             <div className="flex gap-3">
               <div className="bg-muted/30 p-1 rounded-lg border border-border/50 flex items-center">
                 <Button 
                   variant={viewMode === 'grid' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8"
                   onClick={() => setViewMode('grid')}
                 >
                   <LayoutGrid className="w-4 h-4" />
                 </Button>
                 <Button 
                   variant={viewMode === 'table' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8"
                   onClick={() => setViewMode('table')}
                 >
                   <TableIcon className="w-4 h-4" />
                 </Button>
               </div>
               
               {viewMode === 'table' && (
                 <ColumnManager columns={columns} onUpdateColumns={setColumns} />
               )}

               <Separator orientation="vertical" className="h-8 bg-border/50 mx-1" />

               <Button 
                 variant="secondary" 
                 onClick={loadGames} 
                 disabled={loading || syncing}
                 className="gap-2"
               >
                 <RefreshCw className={cn("w-4 h-4", loading && "animate-spin")} />
                 Refresh Library
               </Button>
               
               <Button 
                 onClick={handleSync} 
                 disabled={loading || syncing}
                 className="gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-[0_0_20px_rgba(139,92,246,0.3)]"
               >
                 {syncing ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Play className="w-4 h-4 fill-current" />}
                 Sync to Notion
               </Button>
             </div>
          </div>

          {/* Stats Bar */}
          <div className="grid grid-cols-4 gap-4 shrink-0">
            <div className="p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
              <div className="text-muted-foreground text-xs uppercase tracking-wider mb-1">Total Games</div>
              <div className="text-2xl font-display font-bold">{games.length}</div>
            </div>
            <div className="p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-xs uppercase tracking-wider mb-1">Playtime</div>
               <div className="text-2xl font-display font-bold">
                 {Math.round(games.reduce((acc, g) => acc + g.playtime_forever, 0) / 60)}h
               </div>
            </div>
            <div className="p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-xs uppercase tracking-wider mb-1">Steam Deck Verified</div>
               <div className="text-2xl font-display font-bold text-green-500">
                 {games.filter(g => g.proton?.tier === 'native' || g.proton?.tier === 'platinum').length}
               </div>
            </div>
            <div className="p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-xs uppercase tracking-wider mb-1">Sync Status</div>
               <div className="flex items-center gap-2">
                 <span className="text-2xl font-display font-bold text-yellow-500">
                    {games.filter(g => g.notion_status === 'synced').length}
                 </span>
                 <span className="text-sm text-muted-foreground">/ {games.length}</span>
               </div>
            </div>
          </div>

          {/* View Container */}
          <div className="flex-1 overflow-y-auto pr-2 scrollbar-thin scrollbar-thumb-muted/50 scrollbar-track-transparent">
             {viewMode === 'grid' ? (
               <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pb-10">
                 <AnimatePresence>
                   {games.map((game, i) => (
                     <GameCard key={game.appid} game={game} index={i} />
                   ))}
                 </AnimatePresence>
               </div>
             ) : (
               <div className="pb-10">
                 <GamesTable games={games} columns={columns} />
               </div>
             )}
          </div>
        </div>

        {/* Right Panel: Logs (Collapsible on mobile?) */}
        <div className="w-[350px] shrink-0 hidden xl:flex flex-col gap-4">
           <h3 className="font-display font-bold text-lg text-muted-foreground">Operations Log</h3>
           <SyncLog logs={logs} className="flex-1" />
           
           <div className="p-4 rounded-lg border border-yellow-500/20 bg-yellow-500/5 text-yellow-200/80 text-xs leading-relaxed">
             <div className="flex items-center gap-2 mb-2 text-yellow-400 font-bold">
               <ShieldAlert className="w-4 h-4" />
               <span>Prototype Mode</span>
             </div>
             Actual Steam API calls are blocked by browser CORS policies. This demo simulates the data flow logic using the Python script structure provided.
           </div>
        </div>

      </main>
    </div>
  );
}
