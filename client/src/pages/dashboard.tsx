import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppConfig, GameData, LogEntry, ColumnConfig, DEFAULT_COLUMNS } from "@/lib/types";
import { syncFromSteam, syncToNotion, syncToCraft, getGames, gameToGameData, saveConfig, getConfig } from "@/lib/api-service";
import { GameCard } from "@/components/game-card";
import { GamesTable } from "@/components/games-table";
import { SyncLog } from "@/components/sync-log";
import { ColumnManager } from "@/components/column-manager";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { RefreshCw, Settings, Database, Play, ShieldAlert, LayoutGrid, Table as TableIcon, ScrollText, PenTool } from "lucide-react";
import { toast } from "@/hooks/use-toast";
import { motion, AnimatePresence } from "framer-motion";
import { cn } from "@/lib/utils";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

// Global flag to track if initial load animation has occurred in this session
let hasInitialLoadHappened = false;

export default function Dashboard() {
  const [config, setConfig] = useState<AppConfig>({
    steamKey: "",
    steamId: "",
    notionToken: "",
    notionDbId: "",
    craftUrl: "",
    craftCollectionId: ""
  });
  
  const { data: games = [], isLoading: loading, refetch } = useQuery({
    queryKey: ['games'],
    queryFn: async () => {
      try {
        addLog("Fetching games from database...", "info");
        const backendGames = await getGames();
        const gameData = backendGames.map(gameToGameData);
        addLog(`Loaded ${gameData.length} games from database.`, "success");
        return gameData;
      } catch (error) {
        addLog("Failed to fetch games.", "error");
        return [];
      }
    },
    staleTime: Infinity, // Keep data fresh indefinitely unless manually refreshed
    refetchOnWindowFocus: false,
    refetchOnMount: false // Don't refetch when mounting if we have data
  });

  const [refreshing, setRefreshing] = useState(false);
  const [syncingNotion, setSyncingNotion] = useState(false);
  const [syncingCraft, setSyncingCraft] = useState(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [viewMode, setViewMode] = useState<"grid" | "table">("grid");
  const [columns, setColumns] = useState<ColumnConfig[]>(DEFAULT_COLUMNS);
  const [openSettings, setOpenSettings] = useState(false);
  const [openLogs, setOpenLogs] = useState(false);

  // Load configuration on mount
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const backendConfig = await getConfig();
        setConfig({
          steamKey: backendConfig.steamKey || "",
          steamId: backendConfig.steamId || "",
          notionToken: backendConfig.notionToken || "",
          notionDbId: backendConfig.notionDatabaseId || "",
          craftUrl: backendConfig.craftApiUrl || "",
          craftCollectionId: backendConfig.craftCollectionId || "",
        });
      } catch (error) {
        console.error("Failed to load configuration:", error);
      }
    };
    loadConfig();
  }, []);

  // Set initial load flag when games are loaded
  useEffect(() => {
    if (games.length > 0 && !loading) {
      hasInitialLoadHappened = true;
    }
  }, [games, loading]);

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
    if (!config.steamKey || !config.steamId) {
      toast({
        title: "Configuration Missing",
        description: "Please set your Steam API key and Steam ID in Settings.",
        variant: "destructive"
      });
      setOpenSettings(true);
      return;
    }

    setRefreshing(true);
    try {
      await syncFromSteam((msg, level) => addLog(msg, level));
      // Refetch games from database after syncing
      await refetch();
    } catch (error) {
      addLog("Failed to sync from Steam.", "error");
    } finally {
      setRefreshing(false);
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

    if (games.length === 0) {
      toast({
        title: "No Games to Sync",
        description: "Please refresh your library from Steam first.",
        variant: "destructive"
      });
      return;
    }

    setSyncingNotion(true);
    try {
      await syncToNotion(games, config.notionToken, config.notionDbId, (msg, level) => {
        addLog(msg, level);
      });
    } catch (error) {
      addLog("Failed to sync to Notion.", "error");
      toast({
        title: "Sync Failed",
        description: "Failed to sync to Notion. Check the operations log for details.",
        variant: "destructive"
      });
    } finally {
      setSyncingNotion(false);
    }
  };

  const handleCraftSync = async () => {
    if (!config.craftUrl || !config.craftCollectionId) {
      toast({
        title: "Configuration Missing",
        description: "Please set your Craft API credentials in Settings.",
        variant: "destructive"
      });
      setOpenSettings(true);
      return;
    }

    if (games.length === 0) {
      toast({
        title: "No Games to Sync",
        description: "Please refresh your library from Steam first.",
        variant: "destructive"
      });
      return;
    }

    setSyncingCraft(true);
    try {
      await syncToCraft(games, config.craftUrl, config.craftCollectionId, (msg, level) => {
        addLog(msg, level);
      });
    } catch (error) {
      addLog("Failed to sync to Craft.", "error");
      toast({
        title: "Sync Failed",
        description: "Failed to sync to Craft. Check the operations log for details.",
        variant: "destructive"
      });
    } finally {
      setSyncingCraft(false);
    }
  };

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Header */}
      <header className="border-b border-border/40 bg-background/80 backdrop-blur-md sticky top-0 z-50">
        <div className="container mx-auto px-4 md:px-6 h-14 md:h-16 flex items-center justify-between">
          <div className="flex items-center gap-2 md:gap-3">
            <div className="w-7 h-7 md:w-8 md:h-8 rounded bg-primary/20 border border-primary/50 flex items-center justify-center">
              <Database className="w-3.5 h-3.5 md:w-4 md:h-4 text-primary animate-pulse" />
            </div>
            <h1 className="text-lg md:text-2xl font-display font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white to-white/60">
              GAMESYNC <span className="text-primary text-[10px] md:text-sm align-top">PRO</span>
            </h1>
          </div>

          <div className="flex items-center gap-1 md:gap-4">
            <Dialog open={openLogs} onOpenChange={setOpenLogs}>
              <DialogTrigger asChild>
                <Button variant="ghost" size="icon" className="text-muted-foreground hover:text-foreground h-9 w-9 md:h-auto md:w-auto md:px-3 md:gap-2">
                  <ScrollText className="w-4 h-4" />
                  <span className="hidden md:inline">Operations Log</span>
                </Button>
              </DialogTrigger>
              <DialogContent className="sm:max-w-[600px] border-border/50 bg-card/95 backdrop-blur-xl max-h-[80vh] flex flex-col">
                <DialogHeader>
                  <DialogTitle className="font-display tracking-wide text-xl">Operations Log</DialogTitle>
                  <DialogDescription>
                    Real-time synchronization activity and system events.
                  </DialogDescription>
                </DialogHeader>
                <div className="flex-1 overflow-hidden min-h-[300px] flex flex-col gap-4">
                  <SyncLog logs={logs} className="flex-1 border border-border/50 rounded-md" />
                  <div className="p-4 rounded-lg border border-yellow-500/20 bg-yellow-500/5 text-yellow-200/80 text-xs leading-relaxed shrink-0">
                    <div className="flex items-center gap-2 mb-2 text-yellow-400 font-bold">
                      <ShieldAlert className="w-4 h-4" />
                      <span>Prototype Mode</span>
                    </div>
                    Actual Steam API calls are blocked by browser CORS policies. This demo simulates the data flow logic using the Python script structure provided.
                  </div>
                </div>
              </DialogContent>
            </Dialog>

            <div className="hidden md:flex items-center gap-2 px-3 py-1 rounded-full bg-muted/30 border border-border/50 text-xs font-mono text-muted-foreground">
               <div className="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.5)]" />
               SYSTEM ONLINE
            </div>

            <Dialog open={openSettings} onOpenChange={setOpenSettings}>
              <DialogTrigger asChild>
                <Button variant="outline" size="icon" className="border-border/50 hover:bg-muted/50 h-9 w-9 md:h-auto md:w-auto md:px-3 md:gap-2">
                  <Settings className="w-4 h-4" />
                  <span className="hidden md:inline">Settings</span>
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
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
                        placeholder="XXXXXXXXXXXXXXXX"
                      />
                    </div>
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="steamId" className="text-right text-xs">SteamID64</Label>
                      <Input 
                        id="steamId" 
                        value={config.steamId} 
                        onChange={e => setConfig({...config, steamId: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
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
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
                        placeholder="secret_..."
                      />
                    </div>
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="dbId" className="text-right text-xs">Database ID</Label>
                      <Input 
                        id="dbId" 
                        value={config.notionDbId} 
                        onChange={e => setConfig({...config, notionDbId: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
                        placeholder="32 chars..."
                      />
                    </div>
                  </div>
                  <div className="space-y-2">
                    <h4 className="font-medium text-primary text-sm uppercase tracking-wider">Craft API (Beta)</h4>
                    <Separator className="opacity-50" />
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="craftUrl" className="text-right text-xs">API URL</Label>
                      <Input 
                        id="craftUrl" 
                        value={config.craftUrl || ""} 
                        onChange={e => setConfig({...config, craftUrl: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
                        placeholder="https://connect.craft.do/links/.../api/v1"
                      />
                    </div>
                    <div className="grid grid-cols-4 items-center gap-4">
                      <Label htmlFor="craftCollectionId" className="text-right text-xs">Collection ID</Label>
                      <Input 
                        id="craftCollectionId" 
                        type="password"
                        value={config.craftCollectionId || ""} 
                        onChange={e => setConfig({...config, craftCollectionId: e.target.value})}
                        className="col-span-3 bg-background/50 border-border/50 font-mono text-xs placeholder:text-muted-foreground/40" 
                        placeholder="C8A2B9C7-11DD-44EE-99FF-A1B2C3D4E5F6"
                      />
                    </div>
                  </div>
                </div>
                <DialogFooter>
                  <Button onClick={async () => {
                    try {
                      await saveConfig({
                        steamKey: config.steamKey,
                        steamId: config.steamId,
                        notionToken: config.notionToken,
                        notionDatabaseId: config.notionDbId,
                        craftApiUrl: config.craftUrl,
                        craftCollectionId: config.craftCollectionId,
                      });
                      toast({
                        title: "Settings Saved",
                        description: "Your configuration has been saved successfully.",
                      });
                      setOpenSettings(false);
                    } catch (error) {
                      toast({
                        title: "Save Failed",
                        description: "Failed to save configuration. Please try again.",
                        variant: "destructive",
                      });
                    }
                  }}>Save Changes</Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 min-h-0 container mx-auto px-4 md:px-6 py-4 md:py-8 flex gap-4 md:gap-8 overflow-hidden">
        
        {/* Left Panel: Game Grid */}
        <div className="flex-1 min-h-0 flex flex-col gap-4 md:gap-6 overflow-hidden">
          <div className="flex flex-col md:flex-row md:items-center justify-between gap-3 shrink-0">
             <div className="flex items-center justify-between md:block">
               <div>
                 <h2 className="text-2xl md:text-3xl font-display font-bold">Library</h2>
                 <p className="text-muted-foreground text-xs md:text-sm hidden md:block">Manage your Steam collection sync status.</p>
               </div>
               
               {/* Mobile view toggle */}
               <div className="bg-muted/30 p-1 rounded-lg border border-border/50 flex items-center md:hidden">
                 <Button 
                   variant={viewMode === 'grid' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8 transition-all"
                   onClick={() => setViewMode('grid')}
                 >
                   <LayoutGrid className="w-4 h-4" />
                 </Button>
                 <Button 
                   variant={viewMode === 'table' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8 transition-all"
                   onClick={() => setViewMode('table')}
                 >
                   <TableIcon className="w-4 h-4" />
                 </Button>
               </div>
             </div>
             
             <div className="flex flex-wrap gap-2 md:gap-3">
               <AnimatePresence mode="popLayout">
                 {viewMode === 'table' && (
                   <motion.div
                    initial={{ opacity: 0, scale: 0.9, width: 0 }}
                    animate={{ opacity: 1, scale: 1, width: 'auto' }}
                    exit={{ opacity: 0, scale: 0.9, width: 0 }}
                    transition={{ duration: 0.2 }}
                    className="overflow-hidden hidden md:block"
                   >
                     <ColumnManager columns={columns} onUpdateColumns={setColumns} />
                   </motion.div>
                 )}
               </AnimatePresence>

               {/* Desktop view toggle */}
               <div className="bg-muted/30 p-1 rounded-lg border border-border/50 hidden md:flex items-center">
                 <Button 
                   variant={viewMode === 'grid' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8 transition-all"
                   onClick={() => setViewMode('grid')}
                 >
                   <LayoutGrid className="w-4 h-4" />
                 </Button>
                 <Button 
                   variant={viewMode === 'table' ? 'secondary' : 'ghost'} 
                   size="icon" 
                   className="h-8 w-8 transition-all"
                   onClick={() => setViewMode('table')}
                 >
                   <TableIcon className="w-4 h-4" />
                 </Button>
               </div>

               <Separator orientation="vertical" className="h-8 bg-border/50 mx-1 hidden md:block" />

               <Button 
                 variant="secondary" 
                 onClick={loadGames} 
                 disabled={refreshing}
                 className="gap-2 text-xs md:text-sm flex-1 md:flex-none"
               >
                 <RefreshCw className={cn("w-4 h-4", refreshing && "animate-spin")} />
                 <span className="hidden sm:inline">Refresh Library</span>
                 <span className="sm:hidden">Refresh</span>
               </Button>
               
               {config.craftCollectionId && (
                 <Button 
                   onClick={handleCraftSync} 
                   disabled={syncingCraft}
                   className="gap-2 bg-purple-600 hover:bg-purple-700 text-white shadow-[0_0_20px_rgba(147,51,234,0.3)] text-xs md:text-sm"
                 >
                   {syncingCraft ? <RefreshCw className="w-4 h-4 animate-spin" /> : <PenTool className="w-4 h-4" />}
                   <span className="hidden sm:inline">Sync to Craft</span>
                   <span className="sm:hidden">Craft</span>
                 </Button>
               )}
               
               <Button 
                 onClick={handleSync} 
                 disabled={syncingNotion}
                 className="gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-[0_0_20px_rgba(139,92,246,0.3)] text-xs md:text-sm flex-1 md:flex-none"
               >
                 {syncingNotion ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Play className="w-4 h-4 fill-current" />}
                 <span className="hidden sm:inline">Sync to Notion</span>
                 <span className="sm:hidden">Notion</span>
               </Button>
             </div>
          </div>

          {/* Stats Bar */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-2 md:gap-4 shrink-0">
            <div className="p-3 md:p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
              <div className="text-muted-foreground text-[10px] md:text-xs uppercase tracking-wider mb-1">Total Games</div>
              <div className="text-xl md:text-2xl font-display font-bold">{games.length}</div>
            </div>
            <div className="p-3 md:p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-[10px] md:text-xs uppercase tracking-wider mb-1">Playtime</div>
               <div className="text-xl md:text-2xl font-display font-bold">
                 {Math.round(games.reduce((acc, g) => acc + g.playtime_forever, 0) / 60)}h
               </div>
            </div>
            <div className="p-3 md:p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-[10px] md:text-xs uppercase tracking-wider mb-1">Deck Ready</div>
               <div className="text-xl md:text-2xl font-display font-bold text-green-500">
                 {games.filter(g => g.proton?.tier === 'native' || g.proton?.tier === 'platinum').length}
               </div>
            </div>
            <div className="p-3 md:p-4 rounded-lg bg-card/50 border border-border/50 backdrop-blur-sm">
               <div className="text-muted-foreground text-[10px] md:text-xs uppercase tracking-wider mb-1">Synced</div>
               <div className="flex items-center gap-1 md:gap-2">
                 <span className="text-xl md:text-2xl font-display font-bold text-yellow-500">
                    {games.filter(g => g.notion_status === 'synced').length}
                 </span>
                 <span className="text-xs md:text-sm text-muted-foreground">/ {games.length}</span>
               </div>
            </div>
          </div>

          {/* View Container */}
          <div className="flex-1 min-h-0 overflow-y-auto pr-2 scrollbar-thin scrollbar-thumb-muted/50 scrollbar-track-transparent">
             {viewMode === 'grid' ? (
               <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pb-10">
                 <AnimatePresence>
                   {games.map((game, i) => (
                     <GameCard 
                       key={game.appid} 
                       game={game} 
                       index={i} 
                       enableAnimation={!hasInitialLoadHappened}
                     />
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

      </main>
    </div>
  );
}
