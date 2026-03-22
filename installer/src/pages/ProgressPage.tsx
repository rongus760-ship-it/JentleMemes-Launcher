import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import { Loader } from "lucide-react";

interface Props {
  installPath: string;
  onDone: () => void;
}

export default function ProgressPage({ installPath, onDone }: Props) {
  const [percent, setPercent] = useState(0);
  const [status, setStatus] = useState("Запуск установки...");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const setup = async () => {
      const unlistenProgress = await listen<{
        percent: number;
        status: string;
      }>("install-progress", (ev) => {
        if (cancelled) return;
        setPercent(ev.payload.percent);
        setStatus(ev.payload.status);
        if (ev.payload.percent >= 100) {
          setTimeout(onDone, 800);
        }
      });

      const unlistenError = await listen<string>("install-error", (ev) => {
        if (cancelled) return;
        setError(ev.payload);
      });

      invoke("run_install", { installPath });

      return () => {
        cancelled = true;
        unlistenProgress();
        unlistenError();
      };
    };

    let cleanup: (() => void) | undefined;
    setup().then((fn) => {
      cleanup = fn;
    });

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [installPath, onDone]);

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-10 gap-6">
      {error ? (
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          className="text-center"
        >
          <div className="w-16 h-16 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center mx-auto mb-4">
            <span className="text-2xl">✕</span>
          </div>
          <h3 className="text-lg font-semibold text-red-400">
            Ошибка установки
          </h3>
          <p className="text-sm text-white/40 mt-2 max-w-[400px] break-words">
            {error}
          </p>
        </motion.div>
      ) : (
        <>
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
          >
            <Loader size={32} className="text-jm-accent-light" />
          </motion.div>

          <div className="text-center">
            <h2 className="text-lg font-semibold text-white">Установка</h2>
            <p className="text-sm text-white/40 mt-1">
              Пожалуйста, подождите...
            </p>
          </div>

          <div className="w-full max-w-[400px]">
            <div className="relative h-3 bg-black/40 rounded-full overflow-hidden border border-white/5">
              <motion.div
                className="absolute inset-y-0 left-0 bg-gradient-to-r from-jm-accent to-jm-accent-light rounded-full progress-striped"
                initial={{ width: "0%" }}
                animate={{ width: `${percent}%` }}
                transition={{ duration: 0.3, ease: "easeOut" }}
              />
            </div>
            <div className="flex items-center justify-between mt-2">
              <span className="text-[12px] text-white/30 truncate max-w-[300px]">
                {status}
              </span>
              <span className="text-[12px] text-jm-accent-light font-medium">
                {Math.round(percent)}%
              </span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 mt-2">
            {[0, 1, 2].map((i) => (
              <motion.div
                key={i}
                animate={{ opacity: [0.2, 1, 0.2], scale: [0.8, 1.1, 0.8] }}
                transition={{
                  duration: 1.2,
                  repeat: Infinity,
                  delay: i * 0.2,
                }}
                className="w-1.5 h-1.5 rounded-full bg-jm-accent/50"
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}
