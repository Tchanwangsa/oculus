import { useState, useEffect } from "react";
import { Outlet } from "react-router-dom";
import Sidebar from "@/components/sidebar/Sidebar";
import { JobStatusBar } from "@/components/jobs/JobStatusBar";

const SIDEBAR_KEY = "oculus-sidebar-collapsed";

export default function AppLayout() {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    return localStorage.getItem(SIDEBAR_KEY) === "true";
  });

  useEffect(() => {
    localStorage.setItem(SIDEBAR_KEY, String(collapsed));
  }, [collapsed]);

  return (
    <div className="flex h-full w-full overflow-hidden bg-background">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed((c) => !c)} />
      <main className="flex-1 overflow-hidden flex flex-col min-w-0">
        <div className="flex-1 overflow-hidden">
          <Outlet />
        </div>
        <JobStatusBar />
      </main>
    </div>
  );
}
