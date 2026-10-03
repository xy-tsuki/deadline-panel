import { createContext, Dispatch, PropsWithChildren, SetStateAction, useContext, useState } from "react";
type Tool = "add" | "history" | "cloud" | "import";
const Context = createContext<{ value: Tool | null; set: Dispatch<SetStateAction<Tool | null>> } | null>(null);
export function ToolDisclosure({ children }: PropsWithChildren) {
  const [value, set] = useState<Tool | null>(null);
  return <Context.Provider value={{ value, set }}>{children}</Context.Provider>;
}
export function useToolDisclosure(tool: Tool): [boolean, Dispatch<SetStateAction<boolean>>] {
  const context = useContext(Context);
  const local = useState(false);
  if (!context) return local;
  return [context.value === tool, next => context.set(previous => {
    const open = typeof next === "function" ? next(previous === tool) : next;
    return open ? tool : previous === tool ? null : previous;
  })];
}
