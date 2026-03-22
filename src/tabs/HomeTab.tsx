import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { instanceIconSrc } from "../utils/instanceIcon";
import { Server, Layers, Activity, Users, ArrowRight, Plus, Play, X, Globe, Newspaper } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { AnimatedSection, AnimatedGrid, AnimatedCard, SectionTitle } from "../components/AnimatedSection";

const showToast = (msg: string) => window.dispatchEvent(new CustomEvent("jm_toast", { detail: msg }));

export default function HomeTab({ setActiveTab, openInstance, onLaunchWithServer, onLaunchWorld }: { setActiveTab: (tab: string) => void, openInstance?: (id: string) => void, onLaunchWithServer?: (instanceId: string, serverIp: string) => void, onLaunchWorld?: (instanceId: string, worldName: string) => void }) {
  const [recommendedPacks, setRecommendedPacks] = useState<any[]>([]);
  const [servers, setServers] = useState<any[]>([]);
  const [myInstances, setMyInstances] = useState<any[]>([]);
  const [serverMenu, setServerMenu] = useState<{ ip: string; name: string; motd?: string; players?: number; max?: number; online?: boolean } | null>(null);
  const [serverMenuInstances, setServerMenuInstances] = useState<any[]>([]);
  const [addServerModal, setAddServerModal] = useState(false);
  const [addServerIp, setAddServerIp] = useState("");
  const [addServerName, setAddServerName] = useState("");
  const [addServerError, setAddServerError] = useState("");
  const [lastWorld, setLastWorld] = useState<{ instance_id: string; instance_name: string; world_name: string; last_played?: number } | null>(null);
  const [launcherNews, setLauncherNews] = useState<any[]>([]);
  const [showNews, setShowNews] = useState(true);

  const loadServers = useCallback((instanceId?: string) => {
    invoke("load_servers", { instanceId: instanceId || null })
      .then((serversList: any) => {
        let savedServers = Array.isArray(serversList) ? serversList : [];
        if (savedServers.length === 0) {
          savedServers.push({ ip: "play.jentlememes.ru", name: "JentleMemes Main", last_played: 0, playtime_hours: 0 });
        }
        const srvs = savedServers.map((s: any) => ({ ...s, online: false, players: 0, max: 0, motd: "Загрузка...", icon: s.icon || null }));
        setServers(srvs);
        srvs.forEach((srv: any, index: number) => {
        invoke("ping_server", { ip: srv.ip }).then((data: any) => {
          if (data && data.online) {
            const iconVal = data.icon;
            const icon = (typeof iconVal === "string" && iconVal) ? (iconVal.startsWith("data:") ? iconVal : `data:image/png;base64,${iconVal}`) : null;
            setServers(prev => {
              const newSrvs = [...prev];
              newSrvs[index] = { ...newSrvs[index], online: true, players: data.players?.online || 0, max: data.players?.max || 0, motd: data.motd?.clean?.[0] || "Сервер работает", icon };
              return newSrvs;
            });
          } else {
            setServers(prev => {
              const newSrvs = [...prev];
              newSrvs[index].motd = "Сервер выключен";
              return newSrvs;
            });
          }
        }).catch(() => {
          setServers(prev => {
            const newSrvs = [...prev];
            newSrvs[index].motd = "Ошибка подключения";
            return newSrvs;
          });
        });
      });
    })
    .catch((e) => {
      console.error("load_settings failed:", e);
      setServers([{ ip: "play.jentlememes.ru", name: "JentleMemes Main", last_played: 0, playtime_hours: 0, online: false, players: 0, max: 0, motd: "Ошибка загрузки", icon: null }]);
    });
  }, []);

  useEffect(() => {
    fetch("https://api.modrinth.com/v2/search?limit=4&facets=[[%22project_type:modpack%22]]")
      .then(res => res.json())
      .then(data => setRecommendedPacks(data.hits))
      .catch(console.error);
    invoke("get_instances").then((insts: any) => setMyInstances((insts || []).slice(0, 4))).catch(() => {});
    loadServers();
    invoke("get_last_world", { instanceId: null }).then((w: any) => setLastWorld(w && w.instance_id ? w : null)).catch(() => setLastWorld(null));
    invoke("load_settings").then((s: any) => { setShowNews(s?.show_news !== false); }).catch(() => {});
    invoke("fetch_launcher_news").then((items: any) => setLauncherNews((items || []).slice(0, 3))).catch(() => {});
  }, [loadServers]);

  useEffect(() => {
    if (serverMenu && onLaunchWithServer) {
      invoke("get_instances").then((insts: any) => setServerMenuInstances(insts || [])).catch(() => setServerMenuInstances([]));
    }
  }, [serverMenu, onLaunchWithServer]);

  return (
    <div className="flex flex-col w-full max-w-6xl mx-auto h-full gap-6 pb-10">
      
      {/* СЕРВЕРА */}
      <AnimatedSection delay={0}>
        <div className="flex justify-between items-center mb-4">
          <SectionTitle icon={<Server size={20} />} title="Ваши серверы" />
          <div className="flex gap-2">
            <button onClick={async () => { try { const n = await invoke("import_servers_from_dat", { instanceId: null }); showToast(n ? `Импортировано серверов: ${n}` : "Нет новых серверов"); loadServers(); } catch (e) { showToast(`Ошибка: ${e}`); } }} className="text-sm bg-white/5 hover:bg-white/10 px-3 py-1.5 rounded-lg flex items-center gap-2 transition-colors" title="Импорт из Minecraft (servers.dat)">
              Импорт
            </button>
            <button onClick={() => { setAddServerModal(true); setAddServerIp(""); setAddServerName(""); setAddServerError(""); }} className="text-sm bg-white/5 hover:bg-white/10 px-3 py-1.5 rounded-lg flex items-center gap-2 transition-colors">
              <Plus size={14}/> Добавить
            </button>
          </div>
        </div>
        <AnimatedGrid className="grid grid-cols-1 md:grid-cols-2 gap-4" delay={0.1}>
          {servers.map((srv, i) => (
            <AnimatedCard key={i} className="bg-jm-card border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div onClick={() => onLaunchWithServer ? setServerMenu({ ip: srv.ip, name: srv.name, motd: srv.motd, players: srv.players, max: srv.max, online: srv.online }) : null} className="flex items-center gap-4 hover:border-jm-accent/50 transition-all cursor-pointer hover:-translate-y-1 hover:shadow-lg group min-w-0">
              {srv.icon ? (
                <>
                  <img src={srv.icon} alt="" className="w-10 h-10 md:w-14 md:h-14 rounded-xl object-cover shadow-md shrink-0" style={{ imageRendering: "pixelated" }} onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} />
                  <div className="hidden w-10 h-10 md:w-14 md:h-14 rounded-xl bg-black/50 flex items-center justify-center border border-white/20 shrink-0"><Server size={20} className="text-[var(--text-secondary)]" /></div>
                </>
              ) : (
                <div className="w-10 h-10 md:w-14 md:h-14 rounded-xl bg-black/50 flex items-center justify-center border border-white/5 shrink-0"><Server size={20} className="text-[var(--text-secondary)]" /></div>
              )}
              <div className="flex-grow min-w-0">
                <div className="flex justify-between items-start">
                  <h3 className="font-bold text-white text-sm md:text-base truncate group-hover:text-jm-accent-light transition-colors">{srv.name}</h3>
                  <div className={`flex items-center gap-1 text-[9px] md:text-[10px] font-bold px-1.5 py-0.5 rounded-md uppercase tracking-wider shrink-0 ${srv.online ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>
                    <Activity size={10} /> {srv.online ? 'Online' : 'Offline'}
                  </div>
                </div>
                <p className="text-sm text-[var(--text-secondary)] truncate">{srv.ip}</p>
                <p className="text-xs text-[var(--text-secondary)] truncate mt-1" dangerouslySetInnerHTML={{ __html: srv.motd }}></p>
                {srv.last_played > 0 && (
                  <p className="text-[10px] text-jm-accent/80 mt-0.5">
                    {(() => { const h = Math.floor((Date.now()/1000 - srv.last_played) / 3600); return h < 1 ? "Играл недавно" : h < 24 ? `Играл ${h} ч назад` : `Играл ${Math.floor(h/24)} дн. назад`; })()}
                  </p>
                )}
              </div>
              {srv.online && (
                <div className="flex flex-col items-center justify-center min-w-[60px] bg-black/30 rounded-xl py-2 px-3 border border-white/5">
                  <Users size={16} className="text-jm-accent mb-1" />
                  <span className="text-xs font-bold text-white">{srv.players}/{srv.max}</span>
                </div>
              )}
              </div>
              {srv.last_instance_id && srv.last_instance_name && onLaunchWithServer && (
                <div className="flex items-center justify-between pt-1 border-t border-white/5">
                  <span className="text-[10px] text-[var(--text-secondary)]">Сборка: {srv.last_instance_name}</span>
                  <button onClick={(e) => { e.stopPropagation(); onLaunchWithServer(srv.last_instance_id, srv.ip); setActiveTab("library"); }} className="text-[10px] font-bold text-jm-accent hover:text-jm-accent-light transition-colors">Играть</button>
                </div>
              )}
            </AnimatedCard>
          ))}
        </AnimatedGrid>
      </AnimatedSection>

      {/* ПОСЛЕДНИЙ МИР (1) */}
      {lastWorld && (
        <AnimatedSection delay={0.15}>
          <SectionTitle icon={<Globe size={20} />} title="Последний мир" delay={0.15} />
          <div className="bg-jm-card border border-white/10 rounded-xl p-3 flex flex-col gap-2">
            <div
              onClick={() => openInstance && openInstance(lastWorld.instance_id)}
              className="flex items-center gap-3 hover:border-jm-accent/50 transition-all cursor-pointer hover:-translate-y-1 hover:shadow-lg group min-w-0"
            >
              <div className="w-10 h-10 md:w-14 md:h-14 rounded-xl bg-black/50 flex items-center justify-center border border-white/5 shrink-0">
                <Globe size={22} className="text-jm-accent" />
              </div>
              <div className="flex-grow min-w-0">
                <h3 className="font-bold text-white text-lg truncate group-hover:text-jm-accent-light transition-colors">{lastWorld.world_name}</h3>
                <p className="text-sm text-[var(--text-secondary)] truncate">Сборка: {lastWorld.instance_name}</p>
                {(lastWorld.last_played ?? 0) > 0 && (
                  <p className="text-[10px] text-jm-accent/80 mt-0.5">
                    {(() => { const h = Math.floor((Date.now()/1000 - (lastWorld.last_played ?? 0)) / 3600); return h < 1 ? "Играл недавно" : h < 24 ? `Играл ${h} ч назад` : `Играл ${Math.floor(h/24)} дн. назад`; })()}
                  </p>
                )}
              </div>
              <ArrowRight size={20} className="text-jm-accent opacity-0 group-hover:opacity-100 shrink-0" />
            </div>
            {onLaunchWorld && (
              <div className="flex items-center justify-between pt-1 border-t border-white/5">
                <span className="text-[10px] text-[var(--text-secondary)]">Сборка: {lastWorld.instance_name}</span>
                <button onClick={(e) => { e.stopPropagation(); onLaunchWorld(lastWorld.instance_id, lastWorld.world_name); setActiveTab("library"); }} className="text-[10px] font-bold text-jm-accent hover:text-jm-accent-light transition-colors">Играть</button>
              </div>
            )}
          </div>
        </AnimatedSection>
      )}

      {/* СБОРКИ */}
      {myInstances.length > 0 && (
        <AnimatedSection delay={0.25}>
          <div className="flex justify-between items-center mb-4">
            <SectionTitle icon={<Layers size={20} />} title="Ваши сборки" delay={0.25} />
            <button onClick={() => setActiveTab("library")} className="text-sm text-jm-accent hover:text-jm-accent-light flex items-center gap-1 transition-colors">Все сборки <ArrowRight size={16} /></button>
          </div>
          <AnimatedGrid className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3" delay={0.3}>
            {myInstances.map((inst) => {
              const homeIcon = instanceIconSrc(inst.icon);
              return (
              <AnimatedCard key={inst.id} onClick={() => openInstance ? openInstance(inst.id) : setActiveTab("library")} className="bg-jm-card border border-white/10 rounded-xl p-3 flex items-center gap-3 cursor-pointer group">
                {homeIcon ? (
                  <img src={homeIcon} alt="" className="w-10 h-10 rounded-lg object-cover border border-white/10 shrink-0" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} />
                ) : null}
                <div className={`w-10 h-10 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0 ${homeIcon ? "hidden" : ""}`}>{inst.name?.charAt(0)?.toUpperCase() || "?"}</div>
                <div className="min-w-0">
                  <h3 className="font-bold text-white text-sm truncate group-hover:text-jm-accent-light transition-colors">{inst.name}</h3>
                  <div className="flex gap-2 mt-0.5"><span className="text-xs text-[var(--text-secondary)] capitalize">{inst.loader}</span><span className="text-xs text-[var(--text-secondary)]">{inst.game_version}</span></div>
                </div>
              </AnimatedCard>
            );
            })}
          </AnimatedGrid>
        </AnimatedSection>
      )}

      {/* РЕКОМЕНДАЦИИ */}
      <AnimatedSection delay={0.35}>
        <div className="flex justify-between items-center mb-4">
          <SectionTitle icon={<Layers size={20} />} title="Популярные сборки" delay={0.35} />
          <button onClick={() => setActiveTab("discover")} className="text-sm text-jm-accent hover:text-jm-accent-light flex items-center gap-1 transition-colors">
            Больше сборок <ArrowRight size={16} />
          </button>
        </div>
        <AnimatedGrid className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3" delay={0.4}>
          {recommendedPacks.map((pack) => (
            <AnimatedCard key={pack.project_id} className="bg-jm-card border border-white/10 rounded-xl p-3 flex flex-col cursor-pointer group" onClick={() => setActiveTab("discover")}>
              {pack.icon_url ? (
                <><img src={pack.icon_url} alt="" className="w-12 h-12 rounded-lg object-cover bg-black/50 border border-white/20 mb-2 shadow-md" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} /><div className="hidden w-12 h-12 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-xs font-medium text-white/70 mb-2 shrink-0 self-start">{((pack?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div></>
              ) : (
                <div className="w-12 h-12 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-xs font-medium text-white/70 mb-2 shrink-0 self-start">{((pack?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div>
              )}
              <h3 className="font-bold text-white text-sm md:text-base truncate group-hover:text-jm-accent-light transition-colors">{pack.title}</h3>
              <p className="text-xs text-jm-accent truncate mb-1">от {pack.author}</p>
              <p className="text-xs text-[var(--text-secondary)] line-clamp-2">{pack.description}</p>
            </AnimatedCard>
          ))}
        </AnimatedGrid>
      </AnimatedSection>

      {/* Последние новости */}
      {showNews && launcherNews.length > 0 && (
        <AnimatedSection delay={0.45}>
          <div className="flex justify-between items-center mb-4">
            <SectionTitle icon={<Newspaper size={20} />} title="Последние новости" delay={0.45} />
            <button onClick={() => setActiveTab("news")} className="text-sm text-jm-accent hover:text-jm-accent-light flex items-center gap-1 transition-colors">
              Все новости <ArrowRight size={16} />
            </button>
          </div>
          <AnimatedGrid className="grid grid-cols-1 sm:grid-cols-3 gap-3" delay={0.5}>
            {launcherNews.map((item: any) => (
              <AnimatedCard key={item.id} className="bg-jm-card border border-white/10 rounded-xl overflow-hidden cursor-pointer group" onClick={() => setActiveTab("news")}>
                {item.image && (
                  <div className="h-28 overflow-hidden">
                    <img src={item.image} alt="" className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500" />
                  </div>
                )}
                <div className="p-3">
                  <div className="flex items-center gap-2 mb-1">
                    {item.tag && <span className="text-[10px] px-2 py-0.5 bg-jm-accent/20 text-jm-accent rounded-full font-bold">{item.tag}</span>}
                    <span className="text-[10px]" style={{ color: "var(--text-secondary)" }}>{item.date ? new Date(item.date).toLocaleDateString("ru") : ""}</span>
                  </div>
                  <h4 className="font-bold text-sm truncate">{item.title}</h4>
                  <p className="text-xs mt-1 line-clamp-2" style={{ color: "var(--text-secondary)" }}>{item.body}</p>
                </div>
              </AnimatedCard>
            ))}
          </AnimatedGrid>
        </AnimatedSection>
      )}

      {/* Модалка добавления сервера */}
      <AnimatePresence>
        {addServerModal && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm" onClick={() => setAddServerModal(false)}>
            <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }} onClick={e => e.stopPropagation()} className="bg-jm-card border border-jm-accent/30 rounded-2xl p-6 w-full max-w-md shadow-2xl">
              <div className="flex justify-between items-center mb-4">
                <h3 className="text-xl font-bold text-white">Добавить сервер</h3>
                <button onClick={() => setAddServerModal(false)} className="text-[var(--text-secondary)] hover:text-white p-1 rounded-lg hover:bg-white/10"><X size={20} /></button>
              </div>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-1">IP или адрес</label>
                  <input value={addServerIp} onChange={e => setAddServerIp(e.target.value)} placeholder="play.example.com или 192.168.1.1:25565" className="w-full px-4 py-2.5 rounded-xl bg-black/30 border border-white/10 text-white placeholder-gray-500 focus:border-jm-accent/50 outline-none" />
                </div>
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-1">Название (необязательно)</label>
                  <input value={addServerName} onChange={e => setAddServerName(e.target.value)} placeholder="Мой сервер" className="w-full px-4 py-2.5 rounded-xl bg-black/30 border border-white/10 text-white placeholder-gray-500 focus:border-jm-accent/50 outline-none" />
                </div>
                {addServerError && <p className="text-sm text-red-400">{addServerError}</p>}
              </div>
              <div className="flex gap-2 mt-6">
                <button onClick={() => setAddServerModal(false)} className="flex-1 px-4 py-2.5 rounded-xl bg-white/5 hover:bg-white/10 text-white font-medium transition-colors">Отмена</button>
                <button onClick={async () => {
                  setAddServerError("");
                  try {
                    await invoke("add_recent_server", { ip: addServerIp, name: addServerName, instanceId: null });
                    showToast("Сервер добавлен!");
                    setAddServerModal(false);
                    loadServers();
                  } catch (e: any) {
                    setAddServerError(e?.message || String(e));
                  }
                }} className="flex-1 px-4 py-2.5 rounded-xl bg-jm-accent hover:bg-jm-accent-light text-black font-bold transition-colors">Добавить</button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Модалка выбора сборки для входа на сервер */}
      <AnimatePresence>
        {serverMenu && onLaunchWithServer && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm" onClick={() => setServerMenu(null)}>
            <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }} onClick={e => e.stopPropagation()} className="bg-jm-card border border-jm-accent/30 rounded-2xl p-6 w-full max-w-md shadow-2xl">
              <div className="flex justify-between items-center mb-4">
                <h3 className="text-xl font-bold text-white">Играть на сервере</h3>
                <button onClick={() => setServerMenu(null)} className="text-[var(--text-secondary)] hover:text-white p-1 rounded-lg hover:bg-white/10"><X size={20} /></button>
              </div>
              <div className="mb-4 p-4 rounded-xl bg-black/30 border border-white/5">
                <div className="flex items-center justify-between mb-2">
                  <strong className="text-jm-accent-light">{serverMenu.name}</strong>
                  <span className={`text-xs font-bold px-2 py-1 rounded-md ${serverMenu.online ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>{serverMenu.online ? 'Online' : 'Offline'}</span>
                </div>
                <p className="text-xs text-[var(--text-secondary)] mb-1">{serverMenu.ip}</p>
                {serverMenu.motd && <p className="text-sm text-[var(--text-secondary)] mb-2" dangerouslySetInnerHTML={{ __html: serverMenu.motd }}></p>}
                {serverMenu.online && serverMenu.players !== undefined && <p className="text-xs text-jm-accent">Игроков: {serverMenu.players}/{serverMenu.max ?? "?"}</p>}
              </div>
              <p className="text-sm text-[var(--text-secondary)] mb-4">Выберите сборку для входа:</p>
              <div className="flex flex-col gap-2 max-h-64 overflow-y-auto custom-scrollbar">
                {serverMenuInstances.map((inst) => {
                  const srvIcon = instanceIconSrc(inst.icon);
                  return (
                  <button key={inst.id} onClick={() => { onLaunchWithServer(inst.id, serverMenu.ip); setServerMenu(null); setActiveTab("library"); }} className="flex items-center gap-4 p-3 rounded-xl bg-black/30 border border-white/5 hover:border-jm-accent/50 hover:bg-jm-accent/10 transition-all text-left group">
                    {srvIcon ? (
                      <img src={srvIcon} alt="" className="w-10 h-10 rounded-lg object-cover shrink-0" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} />
                    ) : null}
                    <div className={`w-10 h-10 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0 ${srvIcon ? "hidden" : ""}`}>{inst.name?.charAt(0)?.toUpperCase() || "?"}</div>
                    <div className="min-w-0 flex-1">
                      <div className="font-bold text-white truncate">{inst.name}</div>
                      <div className="text-xs text-[var(--text-secondary)] capitalize">{inst.loader} {inst.game_version}</div>
                    </div>
                    <Play size={18} className="text-jm-accent opacity-0 group-hover:opacity-100 shrink-0" />
                  </button>
                );
                })}
                {serverMenuInstances.length === 0 && <div className="text-[var(--text-secondary)] text-center py-6">Нет сборок. Создайте сборку во вкладке «Сборки».</div>}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

    </div>
  );
}