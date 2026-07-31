import { Navbar } from "./components/layout/Navbar";
import { Sidebar } from "./components/layout/Sidebar";
import { SearchView } from "./components/search/SearchView";
import { BookmarksView } from "./components/session/BookmarksView";
import { DashboardView } from "./components/dashboard/DashboardView";
import { SnippetsView } from "./components/snippets/SnippetsView";
import { SettingsView } from "./components/settings/SettingsView";
import { useApp } from "./context/AppContext";

function TagsView() {
  return (
    <div className="flex-1 flex items-center justify-center">
      <p style={{ color: "var(--text-muted)" }}>Tags — coming in next release</p>
    </div>
  );
}

function MainContent() {
  const { activeView } = useApp();

  return (
    <div className="flex flex-1 overflow-hidden">
      <Sidebar />
      <div className="flex-1 flex overflow-hidden">
        {activeView === "search" && <SearchView />}
        {activeView === "bookmarks" && <BookmarksView />}
        {activeView === "tags" && <TagsView />}
        {activeView === "dashboard" && <DashboardView />}
        {activeView === "snippets" && <SnippetsView />}
        {activeView === "settings" && <SettingsView />}
      </div>
    </div>
  );
}

export default function App() {
  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <Navbar />
      <MainContent />
    </div>
  );
}
