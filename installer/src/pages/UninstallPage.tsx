import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import { Trash2, CheckCircle, AlertTriangle } from "lucide-react";

type Phase = "confirm" | "progress" | "done";

export default function UninstallPage() {
  const [phase, setPhase] = useState<Phase>("confirm");
  const [percent, setPercent] = useState(0);
  const [status, setStatus] = useState("");
  const [error, setError] = useState<string | null>(null);

  const startUninstall = async () => {
    setPhase("progress");

    const unlistenProgress = await listen<{ percent: number; status: string }>(
      "install-progress",
      (ev) => {
        setPercent(ev.payload.percent);
        setStatus(ev.payload.status);
        if (ev.payload.percent >= 100) {
          setTimeout(() => setPhase("done"), 600);
        }
      }
    );

    const unlistenError = await listen<string>("install-error", (ev) => {
      setError(ev.payload);
    });

    invoke("run_uninstall");

    return () => {
      unlistenProgress();
      unlistenError();
    };
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 gap-5">
      <AnimatePresence mode="wait">
        {phase === "confirm" && (
          <motion.div
            key="confirm"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="flex flex-col items-center gap-5"
          >
            <div className="w-20 h-20 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center">
              <AlertTriangle size={36} className="text-red-400" />
            </div>
            <div className="text-center">
              <h2 className="text-xl font-bold text-white">
                Удалить JentleMemes?
              </h2>
              <p className="text-sm text-white/40 mt-1.5 max-w-[340px]">
                Лаунчер и все его файлы будут удалены с вашего компьютера. Данные
                игр (миры, настройки) останутся.
              </p>
            </div>
            <div className="flex items-center gap-3 mt-2">
              <button
                onClick={() => invoke("exit_app")}
                className="px-6 py-2.5 text-sm text-white/50 hover:text-white/80 transition-colors rounded-xl hover:bg-white/5"
              >
                Отмена
              </button>
              <motion.button
                whileHover={{ scale: 1.03 }}
                whileTap={{ scale: 0.97 }}
                onClick={startUninstall}
                className="px-6 py-2.5 bg-red-500/80 hover:bg-red-500 text-white font-semibold rounded-xl text-sm transition-colors shadow-lg shadow-red-500/20"
              >
                Удалить
              </motion.button>
            </div>
          </motion.div>
        )}

        {phase === "progress" && (
          <motion.div
            key="progress"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="flex flex-col items-center gap-5 w-full"
          >
            <motion.div
              animate={{ rotate: 360 }}
              transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
            >
              <Trash2 size={32} className="text-red-400" />
            </motion.div>
            <h2 className="text-lg font-semibold text-white">Удаление...</h2>
            <div className="w-full max-w-[400px]">
              <div className="relative h-3 bg-black/40 rounded-full overflow-hidden border border-white/5">
                <motion.div
                  className="absolute inset-y-0 left-0 bg-gradient-to-r from-red-500 to-red-400 rounded-full"
                  animate={{ width: `${percent}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
              <div className="flex justify-between mt-2 text-[12px]">
                <span className="text-white/30">{status}</span>
                <span className="text-red-400 font-medium">
                  {Math.round(percent)}%
                </span>
              </div>
            </div>
            {error && (
              <p className="text-sm text-red-400 text-center">{error}</p>
            )}
          </motion.div>
        )}

        {phase === "done" && (
          <motion.div
            key="done"
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            className="flex flex-col items-center gap-5"
          >
            <motion.div
              initial={{ scale: 0 }}
              animate={{ scale: 1 }}
              transition={{ type: "spring", stiffness: 200, damping: 15 }}
              className="w-20 h-20 rounded-2xl bg-jm-accent/10 border border-jm-accent/20 flex items-center justify-center"
            >
              <CheckCircle size={36} className="text-jm-accent-light" />
            </motion.div>
            <div className="text-center">
              <h2 className="text-xl font-bold text-white">
                Удаление завершено
              </h2>
              <p className="text-sm text-white/40 mt-1.5">
                JentleMemes Launcher был успешно удалён
              </p>
            </div>
            <motion.button
              whileHover={{ scale: 1.03 }}
              whileTap={{ scale: 0.97 }}
              onClick={() => invoke("exit_app")}
              className="mt-2 px-8 py-3 bg-jm-accent/20 hover:bg-jm-accent/30 text-jm-accent-light font-semibold rounded-xl text-sm transition-colors"
            >
              Закрыть
            </motion.button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
