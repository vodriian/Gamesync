import { useEffect, useRef } from "react";
import { LogEntry } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Terminal } from "lucide-react";

interface SyncLogProps {
  logs: LogEntry[];
  className?: string;
}

export function SyncLog({ logs, className }: SyncLogProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  return (
    <div className={cn("flex flex-col h-full bg-black/90 border border-border/50 rounded-lg overflow-hidden font-mono text-xs shadow-2xl backdrop-blur-xl", className)}>
      <div className="flex items-center gap-2 px-4 py-2 bg-muted/20 border-b border-border/30">
        <Terminal className="w-3.5 h-3.5 text-primary" />
        <span className="text-muted-foreground font-display tracking-wider uppercase text-[10px]">System Log</span>
      </div>
      
      <div 
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1.5 scrollbar-thin scrollbar-thumb-muted scrollbar-track-transparent"
      >
        {logs.length === 0 && (
          <div className="text-muted-foreground/40 italic">Waiting for process initiation...</div>
        )}
        
        {logs.map((log) => (
          <div key={log.id} className="flex gap-3 animate-in fade-in slide-in-from-left-1 duration-200">
            <span className="text-muted-foreground/50 shrink-0 select-none">
              {log.timestamp.toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })}
            </span>
            <span className={cn(
              "break-all",
              log.level === "error" && "text-red-400 font-bold",
              log.level === "warning" && "text-yellow-400",
              log.level === "success" && "text-green-400 font-medium",
              log.level === "info" && "text-slate-300"
            )}>
              {log.message}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
