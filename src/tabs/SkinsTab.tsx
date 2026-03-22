import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { ChevronLeft, ChevronRight, Plus, Check, Trash2, Save, X, Upload, Loader2, Box, Image as ImageIcon, Shirt } from "lucide-react";
import { SkinViewer, WalkingAnimation } from "skinview3d";

function SkinBodyRender({ src, size = 200 }: { src: string; size?: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const img = new Image();
    img.onload = () => {
      const scale = Math.floor(size / 32);
      const w = 16 * scale;
      const h = 32 * scale;
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, w, h);
      ctx.drawImage(img, 8, 8, 8, 8, 4 * scale, 0, 8 * scale, 8 * scale);
      ctx.drawImage(img, 20, 20, 8, 12, 4 * scale, 8 * scale, 8 * scale, 12 * scale);
      ctx.drawImage(img, 44, 20, 4, 12, 0, 8 * scale, 4 * scale, 12 * scale);
      ctx.drawImage(img, 36, 52, 4, 12, 12 * scale, 8 * scale, 4 * scale, 12 * scale);
      ctx.drawImage(img, 4, 20, 4, 12, 4 * scale, 20 * scale, 4 * scale, 12 * scale);
      ctx.drawImage(img, 20, 52, 4, 12, 8 * scale, 20 * scale, 4 * scale, 12 * scale);
      // Overlay layer (head)
      ctx.drawImage(img, 40, 8, 8, 8, 4 * scale, 0, 8 * scale, 8 * scale);
    };
    img.src = src;
  }, [src, size]);
  return <canvas ref={canvasRef} className="drop-shadow-2xl" style={{ imageRendering: "pixelated", width: size * 0.5, height: size }} />;
}

export default function SkinsTab() {
  const [profiles, setProfiles] = useState<any>({ accounts: [], active_account_id: "", skin_presets: [] });
  const [activeAccount, setActiveAccount] = useState<any>(null);
  const [previewSkin, setPreviewSkin] = useState<any>(null);
  const [use3D, setUse3D] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<SkinViewer | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const [editingPreset, setEditingPreset] = useState<any>(null);
  const [editName, setEditName] = useState("");
  const [editUsername, setEditUsername] = useState("");
  const [isUploading, setIsUploading] = useState(false);
  const [editModel, setEditModel] = useState<"default" | "slim">("default");
  const [editCapeUrl, setEditCapeUrl] = useState("");
  const [editCapeType, setEditCapeType] = useState<"none" | "url" | "local">("none");

  async function loadProfiles() {
    const data: any = await invoke("load_profiles");
    if (!data.skin_presets) data.skin_presets = [];
    setProfiles(data);
    const active = data.accounts.find((a: any) => a.id === data.active_account_id);
    setActiveAccount(active || null);
    if (active && !editingPreset) {
      const appliedSkin = data.skin_presets.find((p: any) => p.id === active.active_skin_id);
      if (appliedSkin) setPreviewSkin(appliedSkin);
      else setPreviewSkin({ skin_type: "nickname", skin_data: active.username });
    }
  }

  useEffect(() => {
    loadProfiles();
    const unlisten = listen("profiles_updated", loadProfiles);
    return () => { unlisten.then(f => f()); };
  }, []);

  useEffect(() => {
    if (viewerRef.current) { viewerRef.current.dispose(); viewerRef.current = null; }
    if (!use3D || !canvasRef.current || !previewSkin) return;
    try {
      const skinUrl = previewSkin.skin_type === "local"
        ? previewSkin.skin_data
        : `https://minotar.net/skin/${previewSkin.skin_data || previewSkin.username}.png`;
      const viewer = new SkinViewer({
        canvas: canvasRef.current,
        width: 260,
        height: 360,
        skin: skinUrl,
      });
      viewer.playerObject.skin.modelType = previewSkin.model === "slim" ? "slim" : "default";
      viewer.animation = new WalkingAnimation();
      viewer.autoRotate = true;
      viewer.autoRotateSpeed = 0.5;
      if (previewSkin.cape_url) {
        viewer.loadCape(previewSkin.cape_url);
      }
      viewerRef.current = viewer;
    } catch (e) {
      console.error("Ошибка WebGL:", e);
      setUse3D(false);
    }
    return () => {
      if (viewerRef.current) { viewerRef.current.dispose(); viewerRef.current = null; }
    };
  }, [previewSkin, use3D]);

  const checkScroll = () => {
    if (scrollRef.current) {
      const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
      setCanScrollLeft(scrollLeft > 0);
      setCanScrollRight(Math.ceil(scrollLeft + clientWidth) < scrollWidth - 2);
    }
  };

  useEffect(() => {
    checkScroll();
    window.addEventListener("resize", checkScroll);
    return () => window.removeEventListener("resize", checkScroll);
  }, [profiles.skin_presets, editingPreset]);

  const scroll = (direction: "left" | "right") => {
    if (scrollRef.current) {
      scrollRef.current.scrollBy({ left: direction === "left" ? -200 : 200, behavior: "smooth" });
      setTimeout(checkScroll, 300);
    }
  };

  async function saveProfilesData(newProfiles: any) {
    setProfiles(newProfiles);
    await invoke("save_profiles", { profiles: newProfiles });
    await emit("profiles_updated");
  }

  async function handleApplySkin(presetId: string) {
    if (!activeAccount) return;
    const updatedAccounts = profiles.accounts.map((acc: any) =>
      acc.id === activeAccount.id ? { ...acc, active_skin_id: presetId } : acc
    );
    await saveProfilesData({ ...profiles, accounts: updatedAccounts });
    closeEditor();
  }

  function handleFileUpload(e: any) {
    const file = e.target.files[0];
    if (!file) return;
    setIsUploading(true);
    const reader = new FileReader();
    reader.onload = async (ev: any) => {
      setTimeout(async () => {
        const newPreset = {
          id: "preset_" + Date.now(),
          name: file.name.replace(".png", ""),
          skin_type: "local",
          skin_data: ev.target.result,
          model: "default",
        };
        await saveProfilesData({ ...profiles, skin_presets: [...profiles.skin_presets, newPreset] });
        setIsUploading(false);
        closeEditor();
      }, 800);
    };
    reader.readAsDataURL(file);
  }

  function handleCapeFileUpload(e: any) {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev: any) => {
      setEditCapeUrl(ev.target.result);
      setEditCapeType("local");
      setPreviewSkin((prev: any) => prev ? { ...prev, cape_url: ev.target.result, cape_type: "local" } : prev);
    };
    reader.readAsDataURL(file);
  }

  function openEditor(preset: any = null) {
    if (preset) {
      setEditingPreset(preset);
      setPreviewSkin(preset);
      setEditName(preset.name);
      setEditUsername(preset.skin_data || preset.username || "");
      setEditModel(preset.model || "default");
      setEditCapeUrl(preset.cape_url || "");
      setEditCapeType(preset.cape_type || "none");
    } else {
      setEditingPreset({ id: "new" });
      setEditName("Новый скин");
      setEditUsername("Notch");
      setEditModel("default");
      setEditCapeUrl("");
      setEditCapeType("none");
      setPreviewSkin({ skin_type: "nickname", skin_data: "Notch" });
    }
  }

  function closeEditor() {
    setEditingPreset(null);
    loadProfiles();
  }

  async function handleSaveNickname() {
    if (!editName.trim() || (editingPreset.skin_type !== "local" && !editUsername.trim())) return;
    const capeUrl = editCapeType === "none" ? "" : editCapeUrl;
    let newPresets = [...profiles.skin_presets];
    if (editingPreset.id === "new") {
      newPresets.push({
        id: "preset_" + Date.now(),
        name: editName,
        skin_type: "nickname",
        skin_data: editUsername,
        model: editModel,
        cape_url: capeUrl,
        cape_type: editCapeType,
      });
    } else {
      newPresets = newPresets.map(p =>
        p.id === editingPreset.id
          ? {
              ...p,
              name: editName,
              skin_data: p.skin_type === "local" ? p.skin_data : editUsername,
              model: editModel,
              cape_url: capeUrl,
              cape_type: editCapeType,
            }
          : p
      );
    }
    await saveProfilesData({ ...profiles, skin_presets: newPresets });
    closeEditor();
  }

  async function handleDelete(id: string) {
    const newPresets = profiles.skin_presets.filter((p: any) => p.id !== id);
    let newAccounts = [...profiles.accounts];
    if (activeAccount && activeAccount.active_skin_id === id)
      newAccounts = newAccounts.map(a => a.id === activeAccount.id ? { ...a, active_skin_id: "" } : a);
    await saveProfilesData({ ...profiles, skin_presets: newPresets, accounts: newAccounts });
    closeEditor();
  }

  return (
    <div className="flex flex-col w-full max-w-5xl mx-auto h-full min-h-0 px-6">
      <div className="flex justify-between items-center w-full mb-4 shrink-0">
        <h2 className="text-3xl font-bold text-jm-accent-light">Гардероб</h2>
        <div className="flex bg-black/50 p-1 rounded-xl border border-white/10">
          <button onPointerDown={() => setUse3D(false)} className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${!use3D ? "bg-jm-accent text-black" : "text-[var(--text-secondary)] hover:text-white"}`}>
            <ImageIcon size={14} /> 2D
          </button>
          <button onPointerDown={() => setUse3D(true)} className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${use3D ? "bg-jm-accent text-black" : "text-[var(--text-secondary)] hover:text-white"}`}>
            <Box size={14} /> 3D
          </button>
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-6 w-full flex-grow min-h-0 overflow-y-auto custom-scrollbar">
        {/* Preview */}
        <div className="lg:w-[320px] shrink-0 bg-jm-card p-6 rounded-2xl border border-white/10 shadow-xl flex flex-col items-center justify-center relative overflow-hidden min-h-[340px]">
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(134,168,134,0.15)_0%,transparent_70%)]"></div>
          {previewSkin ? (
            <>
              {use3D ? (
                <canvas ref={canvasRef} className="relative z-10 drop-shadow-2xl max-w-full" />
              ) : (
                <div className="flex flex-col items-center relative z-10">
                  {previewSkin.skin_type === "local" ? (
                    <SkinBodyRender src={previewSkin.skin_data} size={200} />
                  ) : (
                    <img src={`https://mc-heads.net/body/${previewSkin.skin_data || previewSkin.username}/right`} alt="Skin Preview" className="drop-shadow-2xl h-[220px] object-contain transition-all duration-300" />
                  )}
                  {previewSkin.cape_url && (
                    <div className="flex flex-col items-center mt-2">
                      <span className="text-[10px] text-[var(--text-secondary)] mb-0.5">Плащ</span>
                      <img src={previewSkin.cape_url} className="w-[40px] h-auto" style={{ imageRendering: "pixelated" }} alt="Cape" />
                    </div>
                  )}
                </div>
              )}
              <div className="mt-3 text-center relative z-10 bg-black/40 px-4 py-1.5 rounded-xl border border-white/5">
                <div className="text-base font-bold text-white tracking-wide">
                  {previewSkin.name || previewSkin.skin_data || previewSkin.username}
                </div>
              </div>
            </>
          ) : (
            <div className="text-[var(--text-secondary)] z-10">Скин не выбран</div>
          )}
        </div>

        {/* Right panel */}
        <div className="flex-grow bg-jm-card p-6 rounded-2xl border border-white/10 shadow-xl flex flex-col min-h-[340px] min-w-0">
          {!editingPreset ? (
            <>
              <div className="flex justify-between items-center mb-3 shrink-0">
                <h3 className="text-lg font-bold text-white">Ваши пресеты</h3>
                <span className="text-xs text-[var(--text-secondary)]">{profiles.skin_presets.length} сохранено</span>
              </div>
              <div className="relative flex items-center flex-grow min-h-0">
                <button onPointerDown={() => scroll("left")} className={`absolute -left-2 z-20 bg-jm-accent text-black p-1.5 rounded-full shadow-lg transition-all ${canScrollLeft ? "opacity-100 scale-100" : "opacity-0 scale-50 pointer-events-none"}`}>
                  <ChevronLeft size={18} />
                </button>
                <div ref={scrollRef} onScroll={checkScroll} className="flex gap-3 overflow-x-auto snap-x snap-mandatory py-2 px-1 w-full items-center [&::-webkit-scrollbar]:hidden">
                  <div onPointerDown={() => openEditor()} className="snap-center shrink-0 w-[110px] h-[160px] bg-black/40 border-2 border-dashed border-jm-accent/50 rounded-xl flex flex-col items-center justify-center text-jm-accent cursor-pointer hover:bg-jm-accent/10 hover:border-jm-accent transition-all group">
                    <Plus size={28} className="mb-1 group-hover:scale-110 transition-transform" />
                    <span className="font-bold text-xs text-center px-1">Создать</span>
                  </div>
                  {profiles.skin_presets.map((preset: any) => {
                    const isApplied = activeAccount?.active_skin_id === preset.id;
                    return (
                      <div
                        key={preset.id}
                        onPointerDown={() => openEditor(preset)}
                        onMouseEnter={() => setPreviewSkin(preset)}
                        onMouseLeave={() => loadProfiles()}
                        className={`snap-center shrink-0 w-[110px] h-[160px] bg-black/60 rounded-xl p-2 flex flex-col items-center cursor-pointer hover:-translate-y-1 transition-all relative group ${isApplied ? "border-2 border-jm-accent shadow-[0_0_15px_rgba(134,168,134,0.3)]" : "border border-white/10"}`}
                      >
                        {preset.skin_type === "local" ? (
                          <SkinBodyRender src={preset.skin_data} size={80} />
                        ) : (
                          <img src={`https://mc-heads.net/body/${preset.skin_data || preset.username}/right`} className="h-[90px] object-contain drop-shadow-md mb-2" />
                        )}
                        <span className="font-bold text-xs text-white truncate w-full text-center mt-auto">{preset.name}</span>
                        {isApplied && (
                          <div className="absolute -top-1.5 -right-1.5 bg-jm-accent text-black p-0.5 rounded-full shadow-md">
                            <Check size={12} />
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
                <button onPointerDown={() => scroll("right")} className={`absolute -right-2 z-20 bg-jm-accent text-black p-1.5 rounded-full shadow-lg transition-all ${canScrollRight ? "opacity-100 scale-100" : "opacity-0 scale-50 pointer-events-none"}`}>
                  <ChevronRight size={18} />
                </button>
              </div>
            </>
          ) : (
            <div className="flex flex-col h-full animate-in fade-in slide-in-from-right-4 duration-300 overflow-y-auto custom-scrollbar">
              <div className="flex justify-between items-center mb-4 shrink-0">
                <h3 className="text-lg font-bold text-white">
                  {editingPreset.id === "new" ? "Добавить скин" : "Редактирование скина"}
                </h3>
                <button onPointerDown={closeEditor} className="text-[var(--text-secondary)] hover:text-white transition-colors">
                  <X size={20} />
                </button>
              </div>

              <div className="space-y-3 flex-grow min-h-0">
                <div>
                  <label className="text-xs text-[var(--text-secondary)] mb-1 block">Название пресета</label>
                  <input
                    type="text"
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    className="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl px-3 py-2 text-sm text-white outline-none focus:border-jm-accent transition-colors"
                  />
                </div>

                {editingPreset.id === "new" && (
                  <div className="bg-black/30 p-3 rounded-xl border border-white/5">
                    <h4 className="text-jm-accent-light font-bold text-sm mb-2 flex items-center gap-2">
                      <Upload size={14} /> Загрузить из файла
                    </h4>
                    <label className="w-full bg-[var(--input-bg)] border border-dashed border-jm-accent/50 hover:bg-jm-accent/10 rounded-xl px-3 py-4 text-jm-accent flex flex-col items-center justify-center cursor-pointer transition-colors relative text-sm">
                      {isUploading ? (
                        <>
                          <Loader2 size={20} className="animate-spin mb-1" />
                          <span>Загрузка...</span>
                        </>
                      ) : (
                        <>
                          <Upload size={20} className="mb-1" />
                          <span>Нажмите, чтобы выбрать .png файл</span>
                          <input type="file" accept="image/png" className="hidden" onChange={handleFileUpload} disabled={isUploading} />
                        </>
                      )}
                    </label>
                  </div>
                )}

                {(editingPreset.id === "new" || editingPreset.skin_type === "nickname" || !editingPreset.skin_type) && (
                  <div className="bg-black/30 p-3 rounded-xl border border-white/5">
                    <h4 className="text-jm-accent-light font-bold text-sm mb-2">Скин по никнейму</h4>
                    <input
                      type="text"
                      value={editUsername}
                      onChange={(e) => {
                        setEditUsername(e.target.value);
                        setPreviewSkin({ skin_type: "nickname", skin_data: e.target.value || "Steve", model: editModel, cape_url: editCapeType !== "none" ? editCapeUrl : "" });
                      }}
                      placeholder="Никнейм (например: Notch)"
                      className="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl px-3 py-2 text-sm text-white outline-none focus:border-jm-accent transition-colors"
                    />
                  </div>
                )}

                <div className="bg-black/30 p-3 rounded-xl border border-white/5">
                  <h4 className="text-jm-accent-light font-bold text-sm mb-2 flex items-center gap-2">
                    <Shirt size={14} /> Модель рук
                  </h4>
                  <div className="flex gap-2">
                    <button
                      onPointerDown={() => {
                        setEditModel("default");
                        setPreviewSkin((prev: any) => prev ? { ...prev, model: "default" } : prev);
                      }}
                      className={`flex-1 px-3 py-2 rounded-xl text-sm font-bold transition-all ${editModel === "default" ? "bg-jm-accent text-black" : "bg-[var(--input-bg)] text-[var(--text-secondary)] hover:text-white border border-white/10"}`}
                    >
                      Стив (широкие)
                    </button>
                    <button
                      onPointerDown={() => {
                        setEditModel("slim");
                        setPreviewSkin((prev: any) => prev ? { ...prev, model: "slim" } : prev);
                      }}
                      className={`flex-1 px-3 py-2 rounded-xl text-sm font-bold transition-all ${editModel === "slim" ? "bg-jm-accent text-black" : "bg-[var(--input-bg)] text-[var(--text-secondary)] hover:text-white border border-white/10"}`}
                    >
                      Алекс (тонкие)
                    </button>
                  </div>
                </div>

                <div className="bg-black/30 p-3 rounded-xl border border-white/5">
                  <h4 className="text-jm-accent-light font-bold text-sm mb-2">Плащ</h4>
                  <div className="flex gap-2 mb-2">
                    <button
                      onPointerDown={() => {
                        setEditCapeType("none");
                        setEditCapeUrl("");
                        setPreviewSkin((prev: any) => prev ? { ...prev, cape_url: "", cape_type: "none" } : prev);
                      }}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${editCapeType === "none" ? "bg-jm-accent text-black" : "bg-[var(--input-bg)] text-[var(--text-secondary)] hover:text-white border border-white/10"}`}
                    >
                      Без плаща
                    </button>
                    <button
                      onPointerDown={() => setEditCapeType("url")}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${editCapeType === "url" ? "bg-jm-accent text-black" : "bg-[var(--input-bg)] text-[var(--text-secondary)] hover:text-white border border-white/10"}`}
                    >
                      URL
                    </button>
                    <button
                      onPointerDown={() => setEditCapeType("local")}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${editCapeType === "local" ? "bg-jm-accent text-black" : "bg-[var(--input-bg)] text-[var(--text-secondary)] hover:text-white border border-white/10"}`}
                    >
                      Файл
                    </button>
                  </div>
                  {editCapeType === "url" && (
                    <input
                      type="text"
                      value={editCapeUrl}
                      onChange={(e) => {
                        setEditCapeUrl(e.target.value);
                        setPreviewSkin((prev: any) => prev ? { ...prev, cape_url: e.target.value, cape_type: "url" } : prev);
                      }}
                      placeholder="URL плаща (например: https://namemc.com/...)"
                      className="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl px-3 py-2 text-sm text-white outline-none focus:border-jm-accent transition-colors"
                    />
                  )}
                  {editCapeType === "local" && (
                    <label className="w-full bg-[var(--input-bg)] border border-dashed border-jm-accent/50 hover:bg-jm-accent/10 rounded-xl px-3 py-3 text-jm-accent flex flex-col items-center justify-center cursor-pointer transition-colors text-sm">
                      {editCapeUrl ? (
                        <div className="flex flex-col items-center">
                          <img src={editCapeUrl} className="w-[40px] h-auto mb-1" style={{ imageRendering: "pixelated" }} alt="Cape preview" />
                          <span className="text-xs text-[var(--text-secondary)]">Нажмите, чтобы заменить</span>
                        </div>
                      ) : (
                        <>
                          <Upload size={16} className="mb-1" />
                          <span>Выбрать .png файл плаща</span>
                        </>
                      )}
                      <input type="file" accept="image/png" className="hidden" onChange={handleCapeFileUpload} />
                    </label>
                  )}
                </div>
              </div>

              <div className="flex flex-col gap-2 mt-3 shrink-0">
                <button onPointerDown={handleSaveNickname} className="w-full bg-jm-accent hover:bg-jm-accent-light text-black font-bold py-2.5 rounded-xl flex items-center justify-center gap-2 transition-colors text-sm">
                  <Save size={16} /> Сохранить изменения
                </button>
                {editingPreset.id !== "new" && (
                  <div className="flex gap-2">
                    <button onPointerDown={() => handleApplySkin(editingPreset.id)} className="flex-1 bg-white/10 hover:bg-white/20 text-white font-bold py-2.5 rounded-xl flex items-center justify-center gap-2 transition-colors text-sm">
                      <Check size={16} /> Надеть
                    </button>
                    <button onPointerDown={() => handleDelete(editingPreset.id)} className="flex-1 bg-red-500/10 hover:bg-red-500 text-red-500 hover:text-white font-bold py-2.5 rounded-xl flex items-center justify-center transition-colors text-sm">
                      <Trash2 size={16} className="mr-1" /> Удалить
                    </button>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
