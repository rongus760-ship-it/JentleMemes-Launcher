import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { X, Minus } from "lucide-react";
import WelcomePage from "./pages/WelcomePage";
import PathPage from "./pages/PathPage";
import ProgressPage from "./pages/ProgressPage";
import FinishPage from "./pages/FinishPage";
import UninstallPage from "./pages/UninstallPage";

type Page = "welcome" | "path" | "progress" | "finish" | "uninstall";

const pageVariants = {
  enter: { opacity: 0, x: 60, scale: 0.98 },
  center: { opacity: 1, x: 0, scale: 1 },
  exit: { opacity: 0, x: -60, scale: 0.98 },
};

export default function App() {
  const [page, setPage] = useState<Page>("welcome");
  const [installPath, setInstallPath] = useState("");
  const [mode, setMode] = useState<"install" | "uninstall">("install");

  useEffect(() => {
    invoke<string>("get_mode").then((m) => {
      if (m === "uninstall") {
        setMode("uninstall");
        setPage("uninstall");
      }
    });
    invoke<string>("get_default_path").then(setInstallPath);
  }, []);

  const handleDrag = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    invoke("drag_window");
  };

  return (
    <div className="flex flex-col h-screen bg-jm-bg overflow-hidden">
      {/* Ambient glow */}
      <div className="absolute inset-0 pointer-events-none">
        <div className="absolute top-0 left-1/2 -translate-x-1/2 w-[400px] h-[200px] bg-jm-accent/5 rounded-full blur-[100px]" />
        <div className="absolute bottom-0 right-0 w-[200px] h-[200px] bg-jm-accent/3 rounded-full blur-[80px]" />
      </div>

      {/* Titlebar */}
      <div
        onMouseDown={handleDrag}
        className="relative z-10 flex items-center justify-between h-8 px-3 bg-[#070b07] border-b border-white/5 select-none cursor-default"
      >
        <span className="text-[11px] text-white/40 font-medium tracking-wide pointer-events-none">
          {mode === "uninstall"
            ? "JENTLEMEMES — УДАЛЕНИЕ"
            : "JENTLEMEMES — УСТАНОВКА"}
        </span>
        <div className="flex items-center gap-1">
          <button
            className="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10 transition-colors"
            onClick={() => invoke("minimize_window")}
          >
            <Minus size={13} className="text-white/50" />
          </button>
          <button
            className="w-7 h-7 flex items-center justify-center rounded hover:bg-red-500 hover:text-white transition-colors"
            onClick={() => invoke("close_window")}
          >
            <X size={13} className="text-white/50" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="relative flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div
            key={page}
            variants={pageVariants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
            className="absolute inset-0 flex flex-col"
          >
            {page === "welcome" && (
              <WelcomePage onNext={() => setPage("path")} />
            )}
            {page === "path" && (
              <PathPage
                installPath={installPath}
                setInstallPath={setInstallPath}
                onBack={() => setPage("welcome")}
                onNext={() => setPage("progress")}
              />
            )}
            {page === "progress" && (
              <ProgressPage
                installPath={installPath}
                onDone={() => setPage("finish")}
              />
            )}
            {page === "finish" && <FinishPage installPath={installPath} />}
            {page === "uninstall" && <UninstallPage />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
