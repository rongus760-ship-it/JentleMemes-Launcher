import { motion } from "framer-motion";
import { Download } from "lucide-react";

interface Props {
  onNext: () => void;
}

export default function WelcomePage({ onNext }: Props) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 gap-6">
      {/* Animated logo area */}
      <motion.div
        initial={{ scale: 0.5, opacity: 0, rotateY: -30 }}
        animate={{ scale: 1, opacity: 1, rotateY: 0 }}
        transition={{ duration: 0.7, ease: [0.22, 1, 0.36, 1] }}
        className="relative"
      >
        <div className="w-24 h-24 rounded-2xl bg-gradient-to-br from-jm-accent/30 to-jm-accent/5 border border-jm-accent/20 flex items-center justify-center glow-pulse">
          <motion.div
            animate={{ y: [0, -4, 0] }}
            transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
          >
            <Download size={40} className="text-jm-accent-light" />
          </motion.div>
        </div>
        {/* Decorative ring */}
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ delay: 0.3, duration: 0.5 }}
          className="absolute -inset-3 rounded-3xl border border-jm-accent/10"
        />
      </motion.div>

      {/* Title */}
      <motion.div
        initial={{ y: 20, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.2, duration: 0.5 }}
        className="text-center"
      >
        <h1 className="text-2xl font-bold text-white tracking-tight">
          JentleMemes Launcher
        </h1>
        <p className="text-sm text-white/40 mt-1.5">
          Добро пожаловать в установщик
        </p>
      </motion.div>

      {/* Description */}
      <motion.p
        initial={{ y: 20, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.35, duration: 0.5 }}
        className="text-center text-white/50 text-sm max-w-[360px] leading-relaxed"
      >
        Мы установим лаунчер на ваш компьютер. Это займёт всего пару минут.
      </motion.p>

      {/* Install button */}
      <motion.button
        initial={{ y: 20, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.5, duration: 0.5 }}
        whileHover={{ scale: 1.03, boxShadow: "0 0 30px rgba(134, 168, 134, 0.3)" }}
        whileTap={{ scale: 0.97 }}
        onClick={onNext}
        className="mt-2 px-10 py-3 bg-gradient-to-r from-jm-accent to-jm-accent-light text-jm-bg font-semibold rounded-xl text-sm transition-shadow shadow-lg shadow-jm-accent/20"
      >
        Начать установку
      </motion.button>

      {/* Version */}
      <motion.span
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.7 }}
        className="text-[11px] text-white/20 mt-2"
      >
        Версия 1.0.4 - BETA_Prebuild
      </motion.span>
    </div>
  );
}
