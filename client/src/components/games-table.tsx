import { useState } from "react";
import { GameData, ColumnConfig } from "@/lib/types";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { ExternalLink } from "lucide-react";
import { Link } from "wouter";

interface GamesTableProps {
  games: GameData[];
  columns: ColumnConfig[];
}

const TIER_COLORS: Record<string, string> = {
  platinum: "bg-blue-500/20 text-blue-400 border-blue-500/50",
  gold: "bg-yellow-500/20 text-yellow-400 border-yellow-500/50",
  silver: "bg-gray-400/20 text-gray-400 border-gray-400/50",
  bronze: "bg-orange-700/20 text-orange-400 border-orange-700/50",
  borked: "bg-red-600/20 text-red-400 border-red-500/50",
  native: "bg-green-600/20 text-green-400 border-green-500/50",
  unknown: "bg-slate-700/20 text-slate-400 border-slate-600/50",
};

export function GamesTable({ games, columns }: GamesTableProps) {
  const visibleCols = columns.filter(c => c.visible);

  const renderCell = (game: GameData, col: ColumnConfig) => {
    switch (col.id) {
      case "name":
        return (
          <div className="flex items-center gap-2 group/cell">
            <Link href={`/game/${game.appid}`}>
              <div className="flex items-center gap-2 cursor-pointer">
                <img src={game.cover_url} alt="" className="w-8 h-4 object-cover rounded shadow-sm" />
                <span className="font-medium truncate max-w-[200px] group-hover/cell:text-primary transition-colors" title={game.name}>{game.name}</span>
              </div>
            </Link>
            <a 
              href={game.store_url} 
              target="_blank" 
              rel="noreferrer" 
              className="text-muted-foreground/50 hover:text-foreground transition-colors opacity-0 group-hover/cell:opacity-100"
            >
              <ExternalLink className="w-3 h-3" />
            </a>
          </div>
        );
      case "playtime_forever":
        return <span className="font-mono text-muted-foreground">{(game.playtime_forever / 60).toFixed(1)}h</span>;
      case "proton_tier": {
        const tierValue = game.customProperties?.tier || "";
        const tier = game.proton?.tier || "unknown";
        const colorClass = TIER_COLORS[tier] || TIER_COLORS.unknown;
        return (
          <Badge variant="outline" className={cn("text-[10px] h-5 px-1.5 whitespace-nowrap", colorClass)}>
            {tierValue || tier}
          </Badge>
        );
      }
      case "proton_confidence": {
        const confidence = game.customProperties?.confidence || "00 unknown ❓";
        return (
          <span className="text-xs whitespace-nowrap">{confidence}</span>
        );
      }
      case "steam_rating": {
        const rating = game.customProperties?.rating || "00 no reviews 0️⃣";
        return (
          <span className="text-xs whitespace-nowrap">{rating}</span>
        );
      }
      case "notion_status":
        return (
          <div className="flex items-center gap-1.5">
            <div className={cn("w-1.5 h-1.5 rounded-full", 
              game.notion_status === 'synced' ? "bg-green-500 shadow-[0_0_5px_rgba(34,197,94,0.5)]" : 
              game.notion_status === 'error' ? "bg-red-500" : "bg-yellow-500"
            )} />
            <span className="text-xs uppercase tracking-wide text-muted-foreground">{game.notion_status || 'Pending'}</span>
          </div>
        );
      default:
        // Handle user defined columns
        if (col.type === 'select') {
          const value = game.customProperties?.[col.id] || game[col.id];
          return value ? <span className="text-xs">{value}</span> : <span className="text-muted-foreground/30 text-xs italic">-</span>;
        }
        if (col.type === 'multi_select') return <span className="text-muted-foreground/30 text-xs italic">-</span>;
        return <span className="text-muted-foreground text-sm">-</span>;
    }
  };

  return (
    <div className="rounded-md border border-border/50 bg-card/30 backdrop-blur-sm overflow-hidden">
      <Table>
        <TableHeader className="bg-muted/20">
          <TableRow className="border-border/50 hover:bg-transparent">
            {visibleCols.map(col => (
              <TableHead key={col.id} className="text-xs font-display tracking-wider text-muted-foreground uppercase h-10">
                {col.label}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {games.length === 0 ? (
            <TableRow>
              <TableCell colSpan={visibleCols.length} className="h-24 text-center text-muted-foreground">
                No games loaded.
              </TableCell>
            </TableRow>
          ) : (
            games.map((game) => (
              <TableRow key={game.appid} className="border-border/50 hover:bg-muted/10 transition-colors group">
                {visibleCols.map(col => (
                  <TableCell key={col.id} className="py-2.5">
                    {renderCell(game, col)}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}
