import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { ColumnConfig } from "@/lib/types";
import { Columns, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

interface ColumnManagerProps {
  columns: ColumnConfig[];
  onUpdateColumns: (cols: ColumnConfig[]) => void;
}

export function ColumnManager({ columns, onUpdateColumns }: ColumnManagerProps) {
  const [newColName, setNewColName] = useState("");
  const [newColType, setNewColType] = useState<ColumnConfig["type"]>("text");
  const [isAdding, setIsAdding] = useState(false);

  const toggleColumn = (id: string) => {
    onUpdateColumns(columns.map(c => c.id === id ? { ...c, visible: !c.visible } : c));
  };

  const addColumn = () => {
    if (!newColName.trim()) return;
    const newCol: ColumnConfig = {
      id: newColName.toLowerCase().replace(/\s+/g, "_"),
      label: newColName,
      type: newColType,
      visible: true,
      system: false
    };
    onUpdateColumns([...columns, newCol]);
    setNewColName("");
    setIsAdding(false);
  };

  const deleteColumn = (id: string) => {
    onUpdateColumns(columns.filter(c => c.id !== id));
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2 border-dashed border-border/60 hover:border-primary/50">
          <Columns className="w-3.5 h-3.5" />
          Columns
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-80 p-0 bg-card/95 backdrop-blur-xl border-border/50" align="end">
        <div className="p-3 border-b border-border/50 bg-muted/20">
          <h4 className="font-display font-medium text-sm">Table Columns</h4>
          <p className="text-xs text-muted-foreground mt-0.5">Customize your Notion database structure</p>
        </div>
        
        <div className="p-2 max-h-[300px] overflow-y-auto space-y-1">
          {columns.map(col => (
            <div key={col.id} className="flex items-center justify-between p-2 rounded hover:bg-muted/30 group">
              <div className="flex items-center gap-2">
                <Checkbox 
                  id={`col-${col.id}`} 
                  checked={col.visible} 
                  onCheckedChange={() => toggleColumn(col.id)}
                />
                <Label htmlFor={`col-${col.id}`} className="text-sm cursor-pointer font-normal flex items-center gap-2">
                  {col.label}
                  <span className="text-[10px] text-muted-foreground uppercase bg-muted/50 px-1 rounded border border-border/30">
                    {col.type}
                  </span>
                </Label>
              </div>
              {!col.system && (
                <Button 
                  variant="ghost" 
                  size="icon" 
                  className="h-6 w-6 opacity-0 group-hover:opacity-100 text-destructive hover:bg-destructive/10"
                  onClick={() => deleteColumn(col.id)}
                >
                  <Trash2 className="w-3 h-3" />
                </Button>
              )}
            </div>
          ))}
        </div>

        <Separator className="bg-border/50" />
        
        <div className="p-3 bg-muted/10">
          {!isAdding ? (
            <Button 
              variant="ghost" 
              size="sm" 
              className="w-full justify-start text-muted-foreground hover:text-primary"
              onClick={() => setIsAdding(true)}
            >
              <Plus className="w-3.5 h-3.5 mr-2" />
              Add Custom Property
            </Button>
          ) : (
            <div className="space-y-2 animate-in slide-in-from-top-2 duration-200">
              <Input 
                placeholder="Column Name" 
                className="h-8 text-xs bg-background/50" 
                value={newColName}
                onChange={e => setNewColName(e.target.value)}
                autoFocus
              />
              <div className="flex gap-2">
                <Select value={newColType} onValueChange={(v: any) => setNewColType(v)}>
                  <SelectTrigger className="h-8 text-xs flex-1">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="text">Text</SelectItem>
                    <SelectItem value="select">Select</SelectItem>
                    <SelectItem value="multi_select">Multi-Select</SelectItem>
                    <SelectItem value="number">Number</SelectItem>
                    <SelectItem value="url">URL</SelectItem>
                  </SelectContent>
                </Select>
                <Button size="sm" className="h-8 px-3" onClick={addColumn}>Add</Button>
                <Button size="sm" variant="ghost" className="h-8 px-2" onClick={() => setIsAdding(false)}>Cancel</Button>
              </div>
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
