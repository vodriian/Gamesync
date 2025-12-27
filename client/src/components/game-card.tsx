import { motion } from "framer-motion";
import { GameData, ProtonDBInfo } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Clock, ExternalLink } from "lucide-react";
import { Link } from "wouter";

interface GameCardProps {
  game: GameData;
  index: number;
  enableAnimation?: boolean;
}

const TIER_COLORS: Record<string, string> = {
  platinum: "bg-blue-500 hover:bg-blue-600 border-blue-400/50",
  gold: "bg-yellow-500 hover:bg-yellow-600 border-yellow-400/50",
  silver: "bg-gray-400 hover:bg-gray-500 border-gray-300/50",
  bronze: "bg-orange-700 hover:bg-orange-800 border-orange-600/50",
  borked: "bg-red-600 hover:bg-red-700 border-red-500/50",
  native: "bg-green-600 hover:bg-green-700 border-green-500/50",
  unknown: "bg-slate-700 hover:bg-slate-800 border-slate-600/50",
};

export function GameCard({ game, index, enableAnimation = true }: GameCardProps) {
  const tier = game.proton?.tier || "unknown";
  const tierColor = TIER_COLORS[tier] || TIER_COLORS.unknown;

  const content = (
    <Link href={`/game/${game.appid}`}>
      <Card className="group overflow-hidden border-border/50 bg-card/40 backdrop-blur-sm hover:border-primary/50 hover:bg-card/60 transition-all duration-300 cursor-pointer h-full">
        <div className="relative aspect-video overflow-hidden">
          <img 
            src={game.cover_url} 
            alt={game.name}
            className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
            loading="lazy"
          />
          <div className="absolute inset-0 bg-gradient-to-t from-background/90 via-transparent to-transparent opacity-80" />
          
          <div className="absolute bottom-2 left-3 right-3 flex justify-between items-end">
            <Badge className={`${tierColor} text-white uppercase font-bold text-[10px] tracking-wider shadow-[0_0_10px_rgba(0,0,0,0.5)]`}>
              {tier}
            </Badge>
            {game.proton?.score && (
               <span className="text-[10px] font-mono text-white/80 bg-black/50 px-1.5 py-0.5 rounded">
                 {game.proton.score}%
               </span>
            )}
          </div>
        </div>

        <CardContent className="p-4 space-y-3">
          <h3 className="font-display font-bold text-lg leading-tight truncate text-foreground group-hover:text-primary transition-colors">
            {game.name}
          </h3>

          <div className="flex items-center justify-between text-xs text-muted-foreground font-mono">
            <div className="flex items-center gap-1.5">
              <Clock className="w-3.5 h-3.5" />
              <span>{Math.round(game.playtime_forever / 60)}h played</span>
            </div>
            
            <div 
              className="hover:text-foreground transition-colors z-10"
              onClick={(e) => {
                e.preventDefault();
                window.open(game.store_url, '_blank');
              }}
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </div>
          </div>

          <div className="flex items-center gap-2 pt-1">
             <div className={`h-1.5 w-1.5 rounded-full ${game.notion_status === 'synced' ? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]' : game.notion_status === 'error' ? 'bg-red-500' : 'bg-yellow-500'}`} />
             <span className="text-[10px] uppercase tracking-widest text-muted-foreground/80">
               {game.notion_status === 'synced' ? 'Synced' : 'Pending'}
             </span>
          </div>
        </CardContent>
      </Card>
    </Link>
  );

  if (!enableAnimation) {
    return <div>{content}</div>;
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.05, duration: 0.3 }}
    >
      {content}
    </motion.div>
  );
}
