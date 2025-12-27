import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { ColumnConfig } from "@/lib/types";
import { Columns, Plus, Trash2, X, ChevronRight, Pencil } from "lucide-react";
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

interface ColumnManagerProps {
  columns: ColumnConfig[];
  onUpdateColumns: (cols: ColumnConfig[]) => void;
}

export function ColumnManager({ columns, onUpdateColumns }: ColumnManagerProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [newColName, setNewColName] = useState("");
  const [newColType, setNewColType] = useState<ColumnConfig["type"]>("text");
  const [newColOptions, setNewColOptions] = useState<string[]>([]);
  const [newOptionInput, setNewOptionInput] = useState("");
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [open, setOpen] = useState(false);

  const toggleColumn = (id: string) => {
    onUpdateColumns(columns.map(c => c.id === id ? { ...c, visible: !c.visible } : c));
  };

  const addOption = () => {
    if (!newOptionInput.trim()) return;
    if (newColOptions.includes(newOptionInput.trim())) return;
    setNewColOptions([...newColOptions, newOptionInput.trim()]);
    setNewOptionInput("");
  };

  const removeOption = (opt: string) => {
    setNewColOptions(newColOptions.filter(o => o !== opt));
  };

  const startAdding = () => {
    setEditingId(null);
    setNewColName("");
    setNewColType("text");
    setNewColOptions([]);
    setIsFormOpen(true);
  };

  const startEditing = (col: ColumnConfig) => {
    setEditingId(col.id);
    setNewColName(col.label);
    setNewColType(col.type);
    setNewColOptions(col.options || []);
    setIsFormOpen(true);
  };

  const saveColumn = () => {
    if (!newColName.trim()) return;

    const colData: Partial<ColumnConfig> = {
      label: newColName,
      type: newColType,
      options: (newColType === 'select' || newColType === 'multi_select') ? newColOptions : undefined
    };

    if (editingId) {
      // Update existing
      onUpdateColumns(columns.map(c => c.id === editingId ? { ...c, ...colData } : c));
    } else {
      // Add new
      const newCol: ColumnConfig = {
        id: newColName.toLowerCase().replace(/\s+/g, "_") + "_" + Math.random().toString(36).substr(2, 4), // Ensure unique ID
        label: newColName,
        type: newColType,
        visible: true,
        system: false,
        options: colData.options
      };
      onUpdateColumns([...columns, newCol]);
    }

    resetForm();
  };

  const resetForm = () => {
    setNewColName("");
    setNewColType("text");
    setNewColOptions([]);
    setIsFormOpen(false);
    setEditingId(null);
  };

  const deleteColumn = (id: string) => {
    onUpdateColumns(columns.filter(c => c.id !== id));
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
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2 border-dashed border-border/60 hover:border-primary/50">
          <Columns className="w-3.5 h-3.5" />
          Columns
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl gap-0 p-0 overflow-hidden border-border/50 bg-card/95 backdrop-blur-xl">
        <DialogHeader className="p-4 border-b border-border/50 bg-muted/10 flex flex-row items-center justify-between space-y-0 pr-12">
          <div className="space-y-1">
            <DialogTitle className="font-display font-medium text-base">Database Properties</DialogTitle>
            <DialogDescription className="text-xs">
              Manage visible columns and add custom Notion properties.
            </DialogDescription>
          </div>
          
          {!isFormOpen && (
            <Button 
              size="sm" 
              className="h-8 gap-2 bg-primary/20 text-primary hover:bg-primary/30 border border-primary/20"
              onClick={startAdding}
            >
              <Plus className="w-3.5 h-3.5" />
              Add Property
            </Button>
          )}
        </DialogHeader>

        <div className="h-[400px]">
          {!isFormOpen ? (
             <ScrollArea className="h-full p-2">
               <div className="space-y-1">
                 {columns.map(col => (
                   <div key={col.id} className="flex items-center justify-between p-2 rounded hover:bg-muted/30 group transition-colors">
                     <div className="flex items-center gap-3">
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
                              onClick={() => startEditing(col)}
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
                 ))}
               </div>
             </ScrollArea>
          ) : (
             <div className="h-full flex flex-col p-6 animate-in slide-in-from-right-4 duration-200">
                <div className="flex items-center gap-2 mb-6">
                  <Button variant="ghost" size="icon" className="h-8 w-8 -ml-2 rounded-full" onClick={resetForm}>
                    <ChevronRight className="w-5 h-5 rotate-180" />
                  </Button>
                  <h4 className="text-lg font-display font-bold">
                    {editingId ? "Edit Property" : "New Property"}
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
                           onKeyDown={e => e.key === 'Enter' && addOption()}
                         />
                         <Button variant="secondary" onClick={addOption}>
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
                               onClick={() => removeOption(opt)}
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
                  <Button variant="ghost" onClick={resetForm}>Cancel</Button>
                  <Button onClick={saveColumn} disabled={!newColName} className="min-w-[100px]">
                    {editingId ? "Save Changes" : "Create Property"}
                  </Button>
                </div>
             </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
