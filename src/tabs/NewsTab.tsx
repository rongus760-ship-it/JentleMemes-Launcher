import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Newspaper, Pin } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { AnimatedSection, AnimatedGrid, AnimatedCard } from "../components/AnimatedSection";

interface NewsItem {
  id: string;
  title: string;
  body: string;
  image?: string | null;
  date: string;
  tag?: string;
  pinned?: boolean;
}

export default function NewsTab() {
  const [news, setNews] = useState<NewsItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<NewsItem | null>(null);

  useEffect(() => {
    invoke("fetch_launcher_news")
      .then((items: any) => setNews(items || []))
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const tagColors: Record<string, string> = {
    update: "bg-blue-500/20 text-blue-400",
    feature: "bg-green-500/20 text-green-400",
    announcement: "bg-jm-accent/20 text-jm-accent",
    event: "bg-purple-500/20 text-purple-400",
    bugfix: "bg-orange-500/20 text-orange-400",
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <motion.div
          animate={{ rotate: 360 }}
          transition={{ repeat: Infinity, duration: 1, ease: "linear" }}
          className="w-8 h-8 border-2 border-jm-accent border-t-transparent rounded-full"
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col w-full max-w-5xl mx-auto h-full gap-4">
      <AnimatedSection delay={0}>
        <div className="flex items-center gap-3 mb-2">
          <Newspaper size={24} className="text-jm-accent" />
          <h2 className="text-xl md:text-2xl font-bold text-jm-accent-light">Новости</h2>
        </div>
      </AnimatedSection>

      {news.length === 0 ? (
        <AnimatedSection delay={0.1}>
          <div className="text-center py-20" style={{ color: "var(--text-secondary)" }}>
            <Newspaper size={48} className="mx-auto mb-4 opacity-30" />
            <p className="text-lg font-bold">Пока нет новостей</p>
          </div>
        </AnimatedSection>
      ) : (
        <AnimatedGrid className="grid grid-cols-1 md:grid-cols-2 gap-4" delay={0.05}>
          {news.map((item) => (
            <AnimatedCard
              key={item.id}
              className="bg-jm-card rounded-2xl border border-[var(--border)] overflow-hidden cursor-pointer group"
              onClick={() => setSelected(item)}
            >
              {item.image && (
                <div className="h-40 overflow-hidden">
                  <img
                    src={item.image}
                    alt=""
                    className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                  />
                </div>
              )}
              <div className="p-4">
                <div className="flex items-center gap-2 mb-2 flex-wrap">
                  {item.pinned && (
                    <span className="flex items-center gap-1 text-[10px] px-2 py-0.5 bg-jm-accent/20 text-jm-accent rounded-full font-bold">
                      <Pin size={10} /> Закреплено
                    </span>
                  )}
                  {item.tag && (
                    <span className={`text-[10px] px-2 py-0.5 rounded-full font-bold ${tagColors[item.tag] || tagColors.announcement}`}>
                      {item.tag}
                    </span>
                  )}
                  <span className="text-[11px] ml-auto" style={{ color: "var(--text-secondary)" }}>
                    {item.date ? new Date(item.date).toLocaleDateString("ru") : ""}
                  </span>
                </div>
                <h3 className="font-bold text-sm mb-1">{item.title}</h3>
                <p className="text-xs line-clamp-3" style={{ color: "var(--text-secondary)" }}>
                  {item.body}
                </p>
              </div>
            </AnimatedCard>
          ))}
        </AnimatedGrid>
      )}

      {/* Detail modal */}
      <AnimatePresence>
        {selected && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-[9999] bg-black/60 backdrop-blur-sm flex items-center justify-center p-4"
            onClick={() => setSelected(null)}
          >
            <motion.div
              initial={{ scale: 0.9, y: 20 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.9, y: 20 }}
              className="bg-jm-card border border-[var(--border)] rounded-2xl max-w-lg w-full max-h-[80vh] overflow-y-auto shadow-2xl"
              onClick={(e) => e.stopPropagation()}
            >
              {selected.image && (
                <img src={selected.image} alt="" className="w-full h-48 object-cover rounded-t-2xl" />
              )}
              <div className="p-6">
                <div className="flex items-center gap-2 mb-3">
                  {selected.tag && (
                    <span className={`text-xs px-2 py-1 rounded-full font-bold ${tagColors[selected.tag] || tagColors.announcement}`}>
                      {selected.tag}
                    </span>
                  )}
                  <span className="text-xs" style={{ color: "var(--text-secondary)" }}>
                    {selected.date ? new Date(selected.date).toLocaleDateString("ru") : ""}
                  </span>
                </div>
                <h2 className="text-xl font-bold mb-3">{selected.title}</h2>
                <p className="text-sm leading-relaxed whitespace-pre-wrap" style={{ color: "var(--text-secondary)" }}>
                  {selected.body}
                </p>
                <button
                  onClick={() => setSelected(null)}
                  className="mt-4 bg-jm-accent/10 text-jm-accent px-4 py-2 rounded-xl font-bold text-sm hover:bg-jm-accent/20 transition-colors"
                >
                  Закрыть
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
