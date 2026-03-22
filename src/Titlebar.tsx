import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Minus, Square, X, Maximize2 } from "lucide-react";

export default function Titlebar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    invoke<boolean>("window_is_maximized").then(setIsMaximized).catch(() => {});
    const interval = setInterval(() => {
      invoke<boolean>("window_is_maximized").then(setIsMaximized).catch(() => {});
    }, 500);
    return () => clearInterval(interval);
  }, []);

  const handleDrag = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    invoke("window_drag");
  };

  return (
    <div
      onMouseDown={handleDrag}
      className="h-8 flex justify-between items-center select-none border-b border-[var(--border)] shrink-0 z-50"
      style={{ background: "var(--header-bg)" }}
    >
      <div className="text-xs font-bold px-4 flex-grow h-full flex items-center pointer-events-none" style={{ color: "var(--text-secondary)" }}>
        JentleMemes Launcher
      </div>
      <div className="flex h-full">
        <button
          onClick={() => invoke("window_minimize")}
          className="px-4 hover:bg-jm-accent/10 transition-colors h-full flex items-center justify-center"
          style={{ color: "var(--text-secondary)" }}
        >
          <Minus size={14} />
        </button>
        <button
          onClick={() => {
            invoke("window_maximize");
            setTimeout(() => invoke<boolean>("window_is_maximized").then(setIsMaximized), 100);
          }}
          className="px-4 hover:bg-jm-accent/10 transition-colors h-full flex items-center justify-center"
          style={{ color: "var(--text-secondary)" }}
        >
          {isMaximized ? <Square size={12} /> : <Maximize2 size={12} />}
        </button>
        <button
          onClick={() => invoke("window_close")}
          className="px-4 hover:bg-red-500 hover:text-white transition-colors h-full flex items-center justify-center"
          style={{ color: "var(--text-secondary)" }}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
