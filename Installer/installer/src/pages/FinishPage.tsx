import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { CheckCircle, Rocket } from "lucide-react";

interface Props {
  installPath: string;
}

export default function FinishPage({ installPath }: Props) {
  const [launchOnClose, setLaunchOnClose] = useState(true);

  const handleClose = async () => {
    if (launchOnClose) {
      await invoke("launch_app", { installPath });
    }
    try {
      await invoke("exit_app");
    } catch {
      // fallback
      await invoke("close_window");
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 gap-5">
      {/* Success icon */}
      <motion.div
        initial={{ scale: 0, rotate: -180 }}
        animate={{ scale: 1, rotate: 0 }}
        transition={{ type: "spring", stiffness: 200, damping: 15 }}
        className="relative"
      >
        <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-jm-accent/30 to-jm-accent/5 border border-jm-accent/30 flex items-center justify-center">
          <CheckCircle size={36} className="text-jm-accent-light" />
        </div>
        {[...Array(6)].map((_, i) => (
          <motion.div
            key={i}
            initial={{ scale: 0, opacity: 1 }}
            animate={{
              scale: [0, 1],
              opacity: [1, 0],
              x: [0, (i % 2 === 0 ? 1 : -1) * (30 + i * 10)],
              y: [0, -(20 + i * 8)],
            }}
            transition={{ duration: 0.8, delay: 0.3 + i * 0.08 }}
            className="absolute top-1/2 left-1/2 w-2 h-2 rounded-full bg-jm-accent-light"
          />
        ))}
      </motion.div>

      <motion.div
        initial={{ y: 20, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.3, duration: 0.5 }}
        className="text-center"
      >
        <h2 className="text-xl font-bold text-white">Установка завершена!</h2>
        <p className="text-sm text-white/40 mt-1.5">
          JentleMemes Launcher готов к использованию
        </p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.45, duration: 0.4 }}
        className="px-4 py-2.5 bg-black/20 rounded-lg border border-white/5 max-w-[400px] w-full"
      >
        <span className="text-[11px] text-white/30 block">Установлено в:</span>
        <span className="text-[12px] text-white/60 break-all">
          {installPath}
        </span>
      </motion.div>

      <motion.label
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.55, duration: 0.4 }}
        className="flex items-center gap-3 cursor-pointer group"
      >
        <div
          className={`w-5 h-5 rounded-md border-2 flex items-center justify-center transition-all ${
            launchOnClose
              ? "bg-jm-accent border-jm-accent"
              : "border-white/20 group-hover:border-white/40"
          }`}
          onClick={() => setLaunchOnClose(!launchOnClose)}
        >
          {launchOnClose && (
            <motion.svg
              initial={{ scale: 0 }}
              animate={{ scale: 1 }}
              width="12"
              height="12"
              viewBox="0 0 12 12"
              className="text-jm-bg"
            >
              <path
                d="M2 6l3 3 5-5"
                stroke="currentColor"
                strokeWidth="2"
                fill="none"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </motion.svg>
          )}
        </div>
        <span className="text-sm text-white/60 group-hover:text-white/80 transition-colors">
          Запустить лаунчер после закрытия
        </span>
      </motion.label>

      <motion.button
        initial={{ y: 20, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.65, duration: 0.5 }}
        whileHover={{
          scale: 1.03,
          boxShadow: "0 0 30px rgba(134, 168, 134, 0.3)",
        }}
        whileTap={{ scale: 0.97 }}
        onClick={handleClose}
        className="mt-1 px-8 py-3 bg-gradient-to-r from-jm-accent to-jm-accent-light text-jm-bg font-semibold rounded-xl text-sm flex items-center gap-2 shadow-lg shadow-jm-accent/20"
      >
        <Rocket size={16} />
        Готово
      </motion.button>
    </div>
  );
}
