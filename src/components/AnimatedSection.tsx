import { motion } from "framer-motion";
import { ReactNode } from "react";

const sectionVariants = {
  hidden: { opacity: 0, y: 30 },
  visible: { opacity: 1, y: 0 },
};

const cardVariants = {
  hidden: { opacity: 0, y: 20, scale: 0.95 },
  visible: { opacity: 1, y: 0, scale: 1 },
};

export function AnimatedSection({
  children,
  delay = 0,
  className = "",
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  return (
    <motion.div
      variants={sectionVariants}
      initial="hidden"
      animate="visible"
      transition={{ duration: 0.5, delay, ease: [0.22, 1, 0.36, 1] }}
      className={className}
    >
      {children}
    </motion.div>
  );
}

export function AnimatedGrid({
  children,
  className = "",
  delay = 0,
}: {
  children: ReactNode;
  className?: string;
  delay?: number;
}) {
  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={{
        visible: {
          transition: { staggerChildren: 0.08, delayChildren: delay },
        },
      }}
      className={className}
    >
      {children}
    </motion.div>
  );
}

export function AnimatedCard({
  children,
  className = "",
  onClick,
}: {
  children: ReactNode;
  className?: string;
  onClick?: () => void;
}) {
  return (
    <motion.div
      variants={cardVariants}
      whileHover={{ y: -4, scale: 1.01, transition: { duration: 0.2 } }}
      whileTap={{ scale: 0.98 }}
      transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
      className={`card-hover ${className}`}
      onClick={onClick}
    >
      {children}
    </motion.div>
  );
}

export function SectionTitle({
  icon,
  title,
  delay = 0,
}: {
  icon: ReactNode;
  title: string;
  delay?: number;
}) {
  return (
    <motion.h2
      initial={{ opacity: 0, x: -20 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ duration: 0.5, delay, ease: [0.22, 1, 0.36, 1] }}
      className="text-xl md:text-2xl font-bold text-jm-accent-light flex items-center gap-2"
    >
      <motion.span
        animate={{ rotate: [0, -10, 10, 0] }}
        transition={{ duration: 0.6, delay: delay + 0.3 }}
      >
        {icon}
      </motion.span>
      {title}
    </motion.h2>
  );
}
