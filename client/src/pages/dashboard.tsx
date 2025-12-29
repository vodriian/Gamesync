import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppConfig, GameData, LogEntry, ColumnConfig, DEFAULT_COLUMNS } from "@/lib/types";
import { syncFromSteam, syncToNotion, syncToCraft, getGames, gameToGameData, saveConfig, getConfig } from "@/lib/api-service";
import { GameCard } from "@/components/game-card";
import { GamesTable } from "@/components/games-table";
import { SyncLog } from "@/components/sync-log";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { RefreshCw, Settings, Database, Play, LayoutGrid, Table as TableIcon, ScrollText, PenTool, ArrowUp, ArrowDown, Search } from "lucide-react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { toast } from "@/hooks/use-toast";
import { motion, AnimatePresence } from "framer-motion";
import { cn } from "@/lib/utils";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { Reorder } from "framer-motion";
import { Columns, Plus, Trash2, X, ChevronRight, Pencil, GripVertical } from "lucide-react";

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
  const [sortBy, setSortBy] = useState<"name" | "rating" | "tier" | "playtime">("name");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("asc");
  const [searchQuery, setSearchQuery] = useState("");
  const [editingColumnId, setEditingColumnId] = useState<string | null>(null);
  const [newColName, setNewColName] = useState("");
  const [newColType, setNewColType] = useState<ColumnConfig["type"]>("text");
  const [newColOptions, setNewColOptions] = useState<string[]>([]);
  const [newOptionInput, setNewOptionInput] = useState("");
  const [isColumnFormOpen, setIsColumnFormOpen] = useState(false);

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
      toast({
        title: "Library Refreshed",
        description: "Steam library has been updated successfully.",
        variant: "default",
        className: "bg-green-600 text-white border-green-700"
      });
    } catch (error) {
      addLog("Failed to sync from Steam.", "error");
      toast({
        title: "Refresh Failed",
        description: "Failed to refresh Steam library. Check the logs for details.",
        variant: "destructive"
      });
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
      toast({
        title: "Sync Complete",
        description: "Successfully synced games to Notion.",
        variant: "default",
        className: "bg-green-600 text-white border-green-700"
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
      toast({
        title: "Sync Complete",
        description: "Successfully synced games to Craft.",
        variant: "default",
        className: "bg-green-600 text-white border-green-700"
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

  const getTierWeight = (tier?: string) => {
    switch (tier) {
      case 'native': return 6;
      case 'platinum': return 5;
      case 'gold': return 4;
      case 'silver': return 3;
      case 'bronze': return 2;
      case 'borked': return 1;
      default: return 0;
    }
  };

  const filteredGames = searchQuery
    ? games.filter(game => game.name.toLowerCase().includes(searchQuery.toLowerCase()))
    : games;

  const sortedGames = [...filteredGames].sort((a, b) => {
    let comparison = 0;
    switch (sortBy) {
      case 'name':
        comparison = a.name.localeCompare(b.name);
        break;
      case 'rating':
        comparison = (a.review_score || 0) - (b.review_score || 0);
        break;
      case 'tier':
        comparison = getTierWeight(a.proton?.tier) - getTierWeight(b.proton?.tier);
        break;
      case 'playtime':
        comparison = a.playtime_forever - b.playtime_forever;
        break;
    }
    return sortOrder === 'asc' ? comparison : -comparison;
  });

  const handleSort = (field: "name" | "rating" | "tier" | "playtime") => {
    if (sortBy === field) {
      setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
    } else {
      setSortBy(field);
      setSortOrder('asc');
    }
  };

  // Column Manager helpers
  const toggleColumn = (id: string) => {
    setColumns(columns.map(c => c.id === id ? { ...c, visible: !c.visible } : c));
  };

  const addColumnOption = () => {
    if (!newOptionInput.trim()) return;
    if (newColOptions.includes(newOptionInput.trim())) return;
    setNewColOptions([...newColOptions, newOptionInput.trim()]);
    setNewOptionInput("");
  };

  const removeColumnOption = (opt: string) => {
    setNewColOptions(newColOptions.filter(o => o !== opt));
  };

  const startAddingColumn = () => {
    setEditingColumnId(null);
    setNewColName("");
    setNewColType("text");
    setNewColOptions([]);
    setIsColumnFormOpen(true);
  };

  const startEditingColumn = (col: ColumnConfig) => {
    setEditingColumnId(col.id);
    setNewColName(col.label);
    setNewColType(col.type);
    setNewColOptions(col.options || []);
    setIsColumnFormOpen(true);
  };

  const saveColumn = () => {
    if (!newColName.trim()) return;

    const colData: Partial<ColumnConfig> = {
      label: newColName,
      type: newColType,
      options: (newColType === 'select' || newColType === 'multi_select') ? newColOptions : undefined
    };

    if (editingColumnId) {
      setColumns(columns.map(c => c.id === editingColumnId ? { ...c, ...colData } : c));
    } else {
      const newCol: ColumnConfig = {
        id: newColName.toLowerCase().replace(/\s+/g, "_") + "_" + Math.random().toString(36).substr(2, 4),
        label: newColName,
        type: newColType,
        visible: true,
        system: false,
        options: colData.options
      };
      setColumns([...columns, newCol]);
    }

    setIsColumnFormOpen(false);
    setEditingColumnId(null);
    setNewColName("");
    setNewColType("text");
    setNewColOptions([]);
  };

  const resetColumnForm = () => {
    setNewColName("");
    setNewColType("text");
    setNewColOptions([]);
    setIsColumnFormOpen(false);
    setEditingColumnId(null);
  };

  const deleteColumn = (id: string) => {
    setColumns(columns.filter(c => c.id !== id));
  };

  const getTypeColor = (type: string) => {
    switch(type) {
      case 'text': return "text-slate-400 bg-slate-400/10 border-slate-400/20";
      case 'number': return "text-blue-400 bg-blue-400/10 border-blue-400/20";
      case 'select': return "text-purple-400 bg-purple-400/10 border-purple-400/20";
      case 'multi_select': return "text-pink-400 bg-pink-400/10 border-pink-400/20";
      case 'status': return "text-green-400 bg-green-400/10 border-green-400/20";
      case 'url': return "text-cyan-400 bg-cyan-400/10 border-cyan-400/20";
      case 'date': return "text-orange-400 bg-orange-400/10 border-orange-400/20";
      default: return "text-muted-foreground";
    }
  };

  return (
    <div className="h-screen overflow-hidden bg-background text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Header */}
      <header className="border-b border-border/40 bg-background/80 backdrop-blur-md sticky top-0 z-50">
        <div className="container mx-auto px-4 md:px-6 h-16 flex items-center justify-between gap-4">
          <div className="flex items-center gap-2 md:gap-3 shrink-0">
            <div className="w-7 h-7 md:w-8 md:h-8 rounded bg-primary/20 border border-primary/50 flex items-center justify-center">
              <Database className="w-3.5 h-3.5 md:w-4 md:h-4 text-primary animate-pulse" />
            </div>
            <h1 className="text-lg md:text-2xl font-display font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white to-white/60">
              GAMESYNC <span className="text-primary text-[10px] md:text-sm align-top">PRO</span>
            </h1>
          </div>

          {/* Search Bar */}
          <div className="flex-1 max-w-md mx-2 md:mx-4">
            <div className="relative w-full">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <Input
                type="text"
                placeholder="Search games..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 h-9 bg-background/50 border-border/50 text-sm"
              />
            </div>
          </div>

          <div className="flex items-center gap-2 md:gap-3 overflow-x-auto no-scrollbar mask-gradient-right transition-all duration-200">
             {/* View Toggle */}
             <div className="bg-muted/30 p-1 rounded-lg border border-border/50 flex items-center shrink-0">
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

             {/* Sorting */}
             <div className="flex items-center gap-1 shrink-0">
               <Select value={sortBy} onValueChange={(v) => setSortBy(v as any)}>
                 <SelectTrigger className="w-[110px] h-9 text-xs bg-background/50">
                    <SelectValue placeholder="Sort by" />
                 </SelectTrigger>
                 <SelectContent>
                   <SelectItem value="name">Name</SelectItem>
                   <SelectItem value="rating">Rating</SelectItem>
                   <SelectItem value="tier">Tier</SelectItem>
                   <SelectItem value="playtime">Playtime</SelectItem>
                 </SelectContent>
               </Select>
               <Button
                 variant="ghost"
                 size="icon"
                 className="h-9 w-9"
                 onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
               >
                 {sortOrder === 'asc' ? <ArrowUp className="w-4 h-4" /> : <ArrowDown className="w-4 h-4" />}
               </Button>
             </div>

             <Separator orientation="vertical" className="h-6 bg-border/50 hidden md:block" />

             {/* CTAs */}
             <div className="flex items-center gap-2 shrink-0">
               <Button 
                 variant="secondary" 
                 onClick={loadGames} 
                 disabled={refreshing}
                 className="gap-2 text-xs h-9 bg-green-600 hover:bg-green-700 text-white border-green-700"
               >
                 <RefreshCw className={cn("w-3.5 h-3.5", refreshing && "animate-spin")} />
                 <span className="hidden xl:inline">Refresh Library</span>
                 <span className="xl:hidden">Refresh</span>
               </Button>
               
               {config.craftCollectionId && (
                 <Button 
                   onClick={handleCraftSync} 
                   disabled={syncingCraft}
                   className="gap-2 bg-purple-600 hover:bg-purple-700 text-white shadow-[0_0_20px_rgba(147,51,234,0.3)] text-xs h-9"
                 >
                   {syncingCraft ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <PenTool className="w-3.5 h-3.5" />}
                   <span className="hidden xl:inline">Sync to Craft</span>
                   <span className="xl:hidden">Craft</span>
                 </Button>
               )}
               
               <Button 
                 onClick={handleSync} 
                 disabled={syncingNotion}
                 className="gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-[0_0_20px_rgba(139,92,246,0.3)] text-xs h-9"
               >
                 {syncingNotion ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <Play className="w-3.5 h-3.5 fill-current" />}
                 <span className="hidden xl:inline">Sync to Notion</span>
                 <span className="xl:hidden">Notion</span>
               </Button>
             </div>

             <Separator orientation="vertical" className="h-6 bg-border/50 hidden md:block" />

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
                <DialogContent className="sm:max-w-[700px] border-border/50 bg-card/95 backdrop-blur-xl">
                <DialogHeader>
                  <DialogTitle className="font-display tracking-wide text-xl">Configuration</DialogTitle>
                  <DialogDescription>
                    Manage API keys and table view settings.
                  </DialogDescription>
                </DialogHeader>
                <Tabs defaultValue="api" className="w-full">
                  <TabsList className="grid w-full grid-cols-2">
                    <TabsTrigger value="api">API Configuration</TabsTrigger>
                    <TabsTrigger value="columns">Table View</TabsTrigger>
                  </TabsList>
                  <TabsContent value="api" className="mt-4">
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
                  </TabsContent>
                  <TabsContent value="columns" className="mt-4">
                    <div className="space-y-4">
                      <div className="flex items-center justify-between pb-2 border-b border-border/50">
                        <div className="space-y-1">
                          <h3 className="font-display font-medium text-base">Database Properties</h3>
                          <p className="text-xs text-muted-foreground">
                            Manage visible columns and add custom Notion properties.
                          </p>
                        </div>
                        {!isColumnFormOpen && (
                          <Button 
                            size="sm" 
                            className="h-8 gap-2 bg-primary/20 text-primary hover:bg-primary/30 border border-primary/20"
                            onClick={startAddingColumn}
                          >
                            <Plus className="w-3.5 h-3.5" />
                            Add Property
                          </Button>
                        )}
                      </div>
                      <div className="h-[400px]">
                        {!isColumnFormOpen ? (
                          <ScrollArea className="h-full p-2">
                            <Reorder.Group axis="y" values={columns} onReorder={setColumns} className="space-y-1">
                              {columns.map(col => (
                                <Reorder.Item key={col.id} value={col} className="bg-transparent">
                                  <div className="flex items-center justify-between p-2 rounded hover:bg-muted/30 group transition-colors cursor-default select-none">
                                    <div className="flex items-center gap-3">
                                      <div className="cursor-grab active:cursor-grabbing text-muted-foreground/30 hover:text-muted-foreground transition-colors p-1">
                                        <GripVertical className="w-4 h-4" />
                                      </div>
                                      <Checkbox 
                                        id={`col-${col.id}`} 
                                        checked={col.visible} 
                                        onCheckedChange={() => toggleColumn(col.id)}
                                        className="border-border/50 data-[state=checked]:bg-primary data-[state=checked]:border-primary"
                                      />
                                      <div className="flex flex-col gap-0.5">
                                        <Label htmlFor={`col-${col.id}`} className="text-sm cursor-pointer font-medium leading-none">
                                          {col.label}
                                        </Label>
                                        <div className="flex items-center gap-2">
                                          <span className={cn("text-[9px] uppercase tracking-wider px-1 rounded border", getTypeColor(col.type))}>
                                            {col.type.replace('_', ' ')}
                                          </span>
                                          {col.options && col.options.length > 0 && (
                                            <span className="text-[9px] text-muted-foreground">{col.options.length} options</span>
                                          )}
                                        </div>
                                      </div>
                                    </div>
                                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                                      {!col.system && (
                                        <>
                                          <Button 
                                            variant="ghost" 
                                            size="icon" 
                                            className="h-7 w-7 text-muted-foreground hover:text-foreground"
                                            onClick={() => startEditingColumn(col)}
                                          >
                                            <Pencil className="w-3.5 h-3.5" />
                                          </Button>
                                          <Button 
                                            variant="ghost" 
                                            size="icon" 
                                            className="h-7 w-7 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                                            onClick={() => deleteColumn(col.id)}
                                          >
                                            <Trash2 className="w-3.5 h-3.5" />
                                          </Button>
                                        </>
                                      )}
                                    </div>
                                  </div>
                                </Reorder.Item>
                              ))}
                            </Reorder.Group>
                          </ScrollArea>
                        ) : (
                          <div className="h-full flex flex-col p-6 animate-in slide-in-from-right-4 duration-200">
                            <div className="flex items-center gap-2 mb-6">
                              <Button variant="ghost" size="icon" className="h-8 w-8 -ml-2 rounded-full" onClick={resetColumnForm}>
                                <ChevronRight className="w-5 h-5 rotate-180" />
                              </Button>
                              <h4 className="text-lg font-display font-bold">
                                {editingColumnId ? "Edit Property" : "New Property"}
                              </h4>
                            </div>
                            
                            <div className="space-y-6 flex-1">
                              <div className="grid grid-cols-2 gap-6">
                                <div className="space-y-2">
                                  <Label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Property Name</Label>
                                  <Input 
                                    placeholder="e.g. Genre, Finished?" 
                                    className="bg-background/50" 
                                    value={newColName}
                                    onChange={e => setNewColName(e.target.value)}
                                    autoFocus
                                  />
                                </div>

                                <div className="space-y-2">
                                  <Label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Type</Label>
                                  <Select value={newColType} onValueChange={(v: any) => setNewColType(v)}>
                                    <SelectTrigger className="bg-background/50">
                                      <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                      <SelectItem value="text">Text</SelectItem>
                                      <SelectItem value="number">Number</SelectItem>
                                      <SelectItem value="select">Select</SelectItem>
                                      <SelectItem value="multi_select">Multi-Select</SelectItem>
                                      <SelectItem value="date">Date</SelectItem>
                                      <SelectItem value="url">URL</SelectItem>
                                    </SelectContent>
                                  </Select>
                                </div>
                              </div>

                              {(newColType === 'select' || newColType === 'multi_select') && (
                                <div className="space-y-2 pt-4 border-t border-border/30">
                                  <Label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Options</Label>
                                  <div className="flex gap-2">
                                    <Input 
                                      placeholder="Type option and press Enter..." 
                                      className="bg-background/50 flex-1"
                                      value={newOptionInput}
                                      onChange={e => setNewOptionInput(e.target.value)}
                                      onKeyDown={e => e.key === 'Enter' && addColumnOption()}
                                    />
                                    <Button variant="secondary" onClick={addColumnOption}>
                                      <Plus className="w-4 h-4" />
                                    </Button>
                                  </div>
                                  
                                  <div className="flex flex-wrap gap-2 min-h-[80px] p-3 bg-background/30 rounded-lg border border-border/30 content-start">
                                    {newColOptions.length === 0 && (
                                      <span className="text-sm text-muted-foreground/50 italic w-full text-center py-4">No options added yet</span>
                                    )}
                                    {newColOptions.map(opt => (
                                      <Badge key={opt} variant="outline" className="pl-2.5 pr-1.5 h-7 gap-1.5 bg-primary/5 hover:bg-primary/10 transition-colors text-sm">
                                        {opt}
                                        <div 
                                          className="cursor-pointer hover:text-destructive p-0.5 rounded-full hover:bg-destructive/10 transition-colors" 
                                          onClick={() => removeColumnOption(opt)}
                                        >
                                          <X className="w-3 h-3" />
                                        </div>
                                      </Badge>
                                    ))}
                                  </div>
                                </div>
                              )}
                            </div>

                            <div className="pt-6 flex justify-end gap-3 border-t border-border/30 mt-auto">
                              <Button variant="ghost" onClick={resetColumnForm}>Cancel</Button>
                              <Button onClick={saveColumn} disabled={!newColName} className="min-w-[100px]">
                                {editingColumnId ? "Save Changes" : "Create Property"}
                              </Button>
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  </TabsContent>
                </Tabs>
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
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 min-h-0 container mx-auto px-4 md:px-6 pb-4 md:pb-8 flex gap-4 md:gap-8 overflow-hidden">
        
        {/* Left Panel: Game Grid */}
        <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
          {/* Scrollable Content Area */}
          <div className="flex-1 min-h-0 overflow-y-auto -mr-4 pr-4 pt-4 md:pt-6">
            {/* Stats Bar */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-2 md:gap-4 mb-4">
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
            {viewMode === 'grid' ? (
               <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pb-10">
                 <AnimatePresence>
                   {sortedGames.map((game, i) => (
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
                 <GamesTable 
                   games={sortedGames} 
                   columns={columns} 
                   sortBy={sortBy}
                   sortOrder={sortOrder}
                   onSort={handleSort}
                 />
               </div>
             )}
          </div>
        </div>

      </main>
    </div>
  );
}
