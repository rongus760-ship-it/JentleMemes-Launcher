import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

export default function Titlebar() {
  const[isFullscreen, setIsFullscreen] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    const checkFullscreen = async () => setIsFullscreen(await appWindow.isFullscreen());
    checkFullscreen();
    const unlisten = appWindow.onResized(checkFullscreen);
    return () => { unlisten.then(f => f()); };
  },[]);

  if (isFullscreen) return null;

  return (
    <div data-tauri-drag-region className="h-8 bg-[#070b07] flex justify-between items-center select-none border-b border-white/5 shrink-0">
      <div data-tauri-drag-region className="text-xs text-gray-500 font-bold px-4 flex-grow h-full flex items-center">JentleMemes Launcher</div>
      <div className="flex h-full">
        <button onClick={() => appWindow.minimize()} className="px-4 hover:bg-white/10 text-gray-400 transition-colors h-full flex items-center justify-center"><Minus size={14} /></button>
        <button onClick={() => appWindow.toggleMaximize()} className="px-4 hover:bg-white/10 text-gray-400 transition-colors h-full flex items-center justify-center"><Square size={12} /></button>
        <button onClick={() => appWindow.close()} className="px-4 hover:bg-red-500 hover:text-white text-gray-400 transition-colors h-full flex items-center justify-center"><X size={14} /></button>
      </div>
    </div>
  );
}