import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import { FolderOpen, ChevronLeft, ChevronRight, HardDrive } from "lucide-react";

interface Props {
  installPath: string;
  setInstallPath: (p: string) => void;
  onBack: () => void;
  onNext: () => void;
}

export default function PathPage({
  installPath,
  setInstallPath,
  onBack,
  onNext,
}: Props) {
  const [diskInfo, setDiskInfo] = useState<{
    available_gb: number;
    required_gb: number;
    enough: boolean;
  } | null>(null);
  const [pathValid, setPathValid] = useState(true);

  useEffect(() => {
    invoke<{ available_gb: number; required_gb: number; enough: boolean }>(
      "check_disk_space",
      { installPath }
    ).then(setDiskInfo);

    invoke<boolean>("validate_path", { path: installPath }).then(setPathValid);
  }, [installPath]);

  const canProceed = pathValid && (diskInfo?.enough ?? true);

  return (
    <div className="flex-1 flex flex-col px-8 py-6 gap-5">
      {/* Header */}
      <motion.div
        initial={{ y: -10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ duration: 0.4 }}
      >
        <h2 className="text-lg font-semibold text-white">Путь установки</h2>
        <p className="text-sm text-white/40 mt-1">
          Выберите папку для установки лаунчера
        </p>
      </motion.div>

      {/* Path input */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.1, duration: 0.4 }}
        className="flex items-center gap-2"
      >
        <div className="flex-1 relative">
          <input
            type="text"
            value={installPath}
            onChange={(e) => setInstallPath(e.target.value)}
            className={`w-full px-4 py-3 bg-black/40 border rounded-xl text-sm text-white/90 
              focus:border-jm-accent/50 transition-colors ${
                pathValid ? "border-white/10" : "border-red-500/50"
              }`}
          />
          {!pathValid && (
            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[11px] text-red-400">
              Недопустимый путь
            </span>
          )}
        </div>
        <motion.button
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          className="w-11 h-11 flex items-center justify-center rounded-xl bg-jm-accent/10 border border-jm-accent/20 hover:bg-jm-accent/20 transition-colors"
          onClick={async () => {
            const selected = await open({
              directory: true,
              multiple: false,
              title: "Выберите папку для установки",
            });
            if (selected) {
              setInstallPath(selected as string);
            }
          }}
        >
          <FolderOpen size={18} className="text-jm-accent-light" />
        </motion.button>
      </motion.div>

      {/* Disk space info */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.2, duration: 0.4 }}
        className="flex items-center gap-3 px-4 py-3 bg-black/20 rounded-xl border border-white/5"
      >
        <HardDrive size={18} className="text-jm-accent/60" />
        {diskInfo ? (
          <div className="flex-1">
            <div className="flex items-center justify-between text-sm">
              <span className="text-white/50">Свободно на диске</span>
              <span
                className={`font-medium ${
                  diskInfo.enough ? "text-jm-accent-light" : "text-red-400"
                }`}
              >
                {diskInfo.available_gb.toFixed(1)} ГБ
              </span>
            </div>
            <div className="flex items-center justify-between text-[12px] mt-0.5">
              <span className="text-white/30">Требуется</span>
              <span className="text-white/40">
                ~{Math.max(diskInfo.required_gb, 0.05).toFixed(2)} ГБ
              </span>
            </div>
          </div>
        ) : (
          <span className="text-sm text-white/30">Проверка...</span>
        )}
      </motion.div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Navigation */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.3, duration: 0.4 }}
        className="flex items-center justify-between"
      >
        <button
          onClick={onBack}
          className="flex items-center gap-1.5 px-4 py-2.5 text-sm text-white/50 hover:text-white/80 transition-colors rounded-lg hover:bg-white/5"
        >
          <ChevronLeft size={16} />
          Назад
        </button>
        <motion.button
          whileHover={canProceed ? { scale: 1.03 } : {}}
          whileTap={canProceed ? { scale: 0.97 } : {}}
          onClick={canProceed ? onNext : undefined}
          disabled={!canProceed}
          className={`flex items-center gap-1.5 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
            canProceed
              ? "bg-gradient-to-r from-jm-accent to-jm-accent-light text-jm-bg shadow-lg shadow-jm-accent/20"
              : "bg-white/5 text-white/20 cursor-not-allowed"
          }`}
        >
          Установить
          <ChevronRight size={16} />
        </motion.button>
      </motion.div>
    </div>
  );
}
