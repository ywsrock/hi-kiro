import React, { createContext, useContext, useState } from "react";

export type View = "search" | "bookmarks" | "tags" | "dashboard" | "snippets" | "settings";

interface AppContextValue {
  activeView: View;
  setActiveView: (v: View) => void;
  selectedSessionId: string | null;
  setSelectedSessionId: (id: string | null) => void;
  diffSessionIds: [string, string] | null;
  setDiffSessionIds: (ids: [string, string] | null) => void;
  indexing: boolean;
  setIndexing: (v: boolean) => void;
  indexProgress: { processed: number; total: number };
  setIndexProgress: (p: { processed: number; total: number }) => void;
}

const AppContext = createContext<AppContextValue>({
  activeView: "search",
  setActiveView: () => {},
  selectedSessionId: null,
  setSelectedSessionId: () => {},
  diffSessionIds: null,
  setDiffSessionIds: () => {},
  indexing: false,
  setIndexing: () => {},
  indexProgress: { processed: 0, total: 0 },
  setIndexProgress: () => {},
});

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [activeView, setActiveView] = useState<View>("search");
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [diffSessionIds, setDiffSessionIds] = useState<[string, string] | null>(null);
  const [indexing, setIndexing] = useState(true);
  const [indexProgress, setIndexProgress] = useState({ processed: 0, total: 0 });

  return (
    <AppContext.Provider
      value={{
        activeView, setActiveView,
        selectedSessionId, setSelectedSessionId,
        diffSessionIds, setDiffSessionIds,
        indexing, setIndexing,
        indexProgress, setIndexProgress,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  return useContext(AppContext);
}
