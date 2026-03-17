import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { ChevronLeft, ChevronRight, Plus, Check, Trash2, Save, X, Upload, Loader2, Box, Image as ImageIcon } from "lucide-react";
import { SkinViewer, WalkingAnimation } from "skinview3d";

export default function SkinsTab() {
  const[profiles, setProfiles] = useState<any>({ accounts:[], active_account_id: "", skin_presets: [] });
  const[activeAccount, setActiveAccount] = useState<any>(null);
  const[previewSkin, setPreviewSkin] = useState<any>(null);
  const [use3D, setUse3D] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const[canScrollRight, setCanScrollRight] = useState(false);
  const[editingPreset, setEditingPreset] = useState<any>(null);
  const [editName, setEditName] = useState("");
  const[editUsername, setEditUsername] = useState("");
  const [isUploading, setIsUploading] = useState(false);

  async function loadProfiles() {
    const data: any = await invoke("load_profiles");
    if (!data.skin_presets) data.skin_presets =[];
    setProfiles(data);
    const active = data.accounts.find((a: any) => a.id === data.active_account_id);
    setActiveAccount(active || null);
    if (active && !editingPreset) {
      const appliedSkin = data.skin_presets.find((p: any) => p.id === active.active_skin_id);
      if (appliedSkin) setPreviewSkin(appliedSkin);
      else setPreviewSkin({ skin_type: "nickname", skin_data: active.username });
    }
  }

  useEffect(() => { loadProfiles(); const unlisten = listen("profiles_updated", loadProfiles); return () => { unlisten.then(f => f()) }; },[]);

  useEffect(() => {
    if (!use3D || !canvasRef.current || !previewSkin) return;
    let viewer: SkinViewer;
    try {
      const skinUrl = previewSkin.skin_type === "local" ? previewSkin.skin_data : `https://minotar.net/skin/${previewSkin.skin_data || previewSkin.username}.png`;
      viewer = new SkinViewer({ canvas: canvasRef.current, width: 300, height: 400, skin: skinUrl });
      viewer.animation = new WalkingAnimation();
      viewer.autoRotate = true;
      viewer.autoRotateSpeed = 0.5;
    } catch (e) { console.error("Ошибка WebGL:", e); setUse3D(false); }
    return () => { if (viewer) viewer.dispose(); };
  },[previewSkin, use3D]);

  const checkScroll = () => {
    if (scrollRef.current) {
      const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
      setCanScrollLeft(scrollLeft > 0);
      setCanScrollRight(Math.ceil(scrollLeft + clientWidth) < scrollWidth - 2);
    }
  };

  useEffect(() => { checkScroll(); window.addEventListener("resize", checkScroll); return () => window.removeEventListener("resize", checkScroll); },[profiles.skin_presets, editingPreset]);

  const scroll = (direction: "left" | "right") => {
    if (scrollRef.current) {
      scrollRef.current.scrollBy({ left: direction === "left" ? -250 : 250, behavior: "smooth" });
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
    const updatedAccounts = profiles.accounts.map((acc: any) => acc.id === activeAccount.id ? { ...acc, active_skin_id: presetId } : acc);
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
        const newPreset = { id: "preset_" + Date.now(), name: file.name.replace(".png", ""), skin_type: "local", skin_data: ev.target.result };
        await saveProfilesData({ ...profiles, skin_presets:[...profiles.skin_presets, newPreset] });
        setIsUploading(false);
        closeEditor();
      }, 800);
    };
    reader.readAsDataURL(file);
  }

  function openEditor(preset: any = null) {
    if (preset) { 
      setEditingPreset(preset); 
      setPreviewSkin(preset); 
      setEditName(preset.name); 
      setEditUsername(preset.skin_data || preset.username || ""); 
    } 
    else { 
      setEditingPreset({ id: "new" }); 
      setEditName("Новый скин"); 
      setEditUsername("Notch"); 
      setPreviewSkin({ skin_type: "nickname", skin_data: "Notch" }); 
    }
  }

  function closeEditor() { setEditingPreset(null); loadProfiles(); }

  async function handleSaveNickname() {
    if (!editName.trim() || (editingPreset.skin_type !== "local" && !editUsername.trim())) return;
    let newPresets =[...profiles.skin_presets];
    if (editingPreset.id === "new") {
      newPresets.push({ id: "preset_" + Date.now(), name: editName, skin_type: "nickname", skin_data: editUsername });
    } else {
      newPresets = newPresets.map(p => p.id === editingPreset.id ? { ...p, name: editName, skin_data: p.skin_type === "local" ? p.skin_data : editUsername } : p);
    }
    await saveProfilesData({ ...profiles, skin_presets: newPresets });
    closeEditor();
  }

  async function handleDelete(id: string) {
    const newPresets = profiles.skin_presets.filter((p: any) => p.id !== id);
    let newAccounts = [...profiles.accounts];
    if (activeAccount && activeAccount.active_skin_id === id) newAccounts = newAccounts.map(a => a.id === activeAccount.id ? { ...a, active_skin_id: "" } : a);
    await saveProfilesData({ ...profiles, skin_presets: newPresets, accounts: newAccounts });
    closeEditor();
  }

  return (
    <div className="flex flex-col items-center w-full max-w-5xl mx-auto h-full">
      <div className="flex justify-between items-center w-full mb-6">
        <h2 className="text-3xl font-bold text-jm-accent-light">Гардероб</h2>
        <div className="flex bg-black/50 p-1 rounded-xl border border-white/10">
          <button onClick={() => setUse3D(false)} className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-bold transition-all ${!use3D ? "bg-jm-accent text-black" : "text-gray-400 hover:text-white"}`}><ImageIcon size={16} /> 2D</button>
          <button onClick={() => setUse3D(true)} className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-bold transition-all ${use3D ? "bg-jm-accent text-black" : "text-gray-400 hover:text-white"}`}><Box size={16} /> 3D</button>
        </div>
      </div>
      <div className="flex gap-8 w-full h-[500px]">
        <div className="w-1/3 bg-jm-card p-6 rounded-3xl border border-white/10 shadow-xl flex flex-col items-center justify-center relative overflow-hidden">
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(134,168,134,0.15)_0%,transparent_70%)]"></div>
          {previewSkin ? (
            <>
              {use3D ? <canvas ref={canvasRef} className="relative z-10 drop-shadow-2xl" /> : (
                previewSkin.skin_type === "local" ? <img src={previewSkin.skin_data} alt="Local Skin" className="relative z-10 drop-shadow-2xl w-[200px] h-[200px] object-contain" style={{ imageRendering: "pixelated" }} /> : <img src={`https://mc-heads.net/body/${previewSkin.skin_data || previewSkin.username}/right`} alt="Skin Preview" className="relative z-10 drop-shadow-2xl h-[320px] object-contain transition-all duration-300" />
              )}
              <div className="mt-6 text-center relative z-10 bg-black/40 px-6 py-2 rounded-xl border border-white/5"><div className="text-xl font-bold text-white tracking-wide">{previewSkin.name || previewSkin.skin_data || previewSkin.username}</div></div>
            </>
          ) : <div className="text-gray-500 z-10">Скин не выбран</div>}
        </div>

        <div className="w-2/3 bg-jm-card p-6 rounded-3xl border border-white/10 shadow-xl flex flex-col">
          {!editingPreset ? (
            <>
              <div className="flex justify-between items-center mb-6"><h3 className="text-xl font-bold text-white">Ваши пресеты</h3><span className="text-sm text-gray-400">{profiles.skin_presets.length} сохранено</span></div>
              <div className="relative flex items-center flex-grow">
                <button onClick={() => scroll("left")} className={`absolute -left-4 z-20 bg-jm-accent text-black p-2 rounded-full shadow-lg transition-all ${canScrollLeft ? "opacity-100 scale-100" : "opacity-0 scale-50 pointer-events-none"}`}><ChevronLeft size={24} /></button>
                <div ref={scrollRef} onScroll={checkScroll} className="flex gap-4 overflow-x-auto snap-x snap-mandatory py-4 px-2 w-full h-full items-center [&::-webkit-scrollbar]:hidden">
                  <div onClick={() => openEditor()} className="snap-center shrink-0 w-[140px] h-[200px] bg-black/40 border-2 border-dashed border-jm-accent/50 rounded-2xl flex flex-col items-center justify-center text-jm-accent cursor-pointer hover:bg-jm-accent/10 hover:border-jm-accent transition-all group"><Plus size={40} className="mb-2 group-hover:scale-110 transition-transform" /><span className="font-bold text-sm text-center px-2">Создать</span></div>
                  {profiles.skin_presets.map((preset: any) => {
                    const isApplied = activeAccount?.active_skin_id === preset.id;
                    return (
                      <div key={preset.id} onClick={() => openEditor(preset)} onMouseEnter={() => setPreviewSkin(preset)} onMouseLeave={() => loadProfiles()} className={`snap-center shrink-0 w-[140px] h-[200px] bg-black/60 rounded-2xl p-3 flex flex-col items-center cursor-pointer hover:-translate-y-2 transition-all relative group ${isApplied ? 'border-2 border-jm-accent shadow-[0_0_15px_rgba(134,168,134,0.3)]' : 'border border-white/10'}`}>
                        {preset.skin_type === "local" ? <img src={preset.skin_data} className="w-[80px] h-[80px] object-contain drop-shadow-md mb-auto mt-4" style={{ imageRendering: "pixelated" }} /> : <img src={`https://mc-heads.net/body/${preset.skin_data || preset.username}/right`} className="h-[120px] object-contain drop-shadow-md mb-3" />}
                        <span className="font-bold text-sm text-white truncate w-full text-center mt-auto">{preset.name}</span>
                        {isApplied && <div className="absolute -top-2 -right-2 bg-jm-accent text-black p-1 rounded-full shadow-md"><Check size={14} /></div>}
                      </div>
                    );
                  })}
                </div>
                <button onClick={() => scroll("right")} className={`absolute -right-4 z-20 bg-jm-accent text-black p-2 rounded-full shadow-lg transition-all ${canScrollRight ? "opacity-100 scale-100" : "opacity-0 scale-50 pointer-events-none"}`}><ChevronRight size={24} /></button>
              </div>
            </>
          ) : (
            <div className="flex flex-col h-full animate-in fade-in slide-in-from-right-4 duration-300">
              <div className="flex justify-between items-center mb-6"><h3 className="text-xl font-bold text-white">{editingPreset.id === "new" ? "Добавить скин" : "Редактирование скина"}</h3><button onClick={closeEditor} className="text-gray-400 hover:text-white transition-colors"><X size={24} /></button></div>
              
              <div className="space-y-4 flex-grow">
                <div>
                  <label className="text-sm text-gray-400 mb-1 block">Название пресета</label>
                  <input type="text" value={editName} onChange={(e) => setEditName(e.target.value)} className="w-full bg-[#0b110b] border border-white/10 rounded-xl px-4 py-2 text-white outline-none focus:border-jm-accent transition-colors" />
                </div>

                {editingPreset.id === "new" && (
                  <div className="bg-black/30 p-4 rounded-xl border border-white/5 mt-4">
                    <h4 className="text-jm-accent-light font-bold mb-3 flex items-center gap-2"><Upload size={18} /> Загрузить из файла</h4>
                    <label className="w-full bg-[#0b110b] border border-dashed border-jm-accent/50 hover:bg-jm-accent/10 rounded-xl px-4 py-6 text-jm-accent flex flex-col items-center justify-center cursor-pointer transition-colors relative">
                      {isUploading ? <><Loader2 size={24} className="animate-spin mb-2" /><span>Загрузка...</span></> : <><Upload size={24} className="mb-2" /><span>Нажмите, чтобы выбрать .png файл</span><input type="file" accept="image/png" className="hidden" onChange={handleFileUpload} disabled={isUploading} /></>}
                    </label>
                  </div>
                )}

                {(editingPreset.id === "new" || editingPreset.skin_type === "nickname" || !editingPreset.skin_type) && (
                  <div className="bg-black/30 p-4 rounded-xl border border-white/5 mt-4">
                    <h4 className="text-jm-accent-light font-bold mb-3">Скин по никнейму</h4>
                    <input type="text" value={editUsername} onChange={(e) => { setEditUsername(e.target.value); setPreviewSkin({ skin_type: "nickname", skin_data: e.target.value || "Steve" }); }} placeholder="Никнейм (например: Notch)" className="w-full bg-[#0b110b] border border-white/10 rounded-xl px-4 py-2 text-white outline-none focus:border-jm-accent transition-colors" />
                  </div>
                )}
              </div>

              <div className="flex flex-col gap-3 mt-4">
                <button onClick={handleSaveNickname} className="w-full bg-jm-accent hover:bg-jm-accent-light text-black font-bold py-3 rounded-xl flex items-center justify-center gap-2 transition-colors">
                  <Save size={18} /> Сохранить изменения
                </button>
                {editingPreset.id !== "new" && (
                  <div className="flex gap-3">
                    <button onClick={() => handleApplySkin(editingPreset.id)} className="flex-1 bg-white/10 hover:bg-white/20 text-white font-bold py-3 rounded-xl flex items-center justify-center gap-2 transition-colors">
                      <Check size={18} /> Надеть
                    </button>
                    <button onClick={() => handleDelete(editingPreset.id)} className="flex-1 bg-red-500/10 hover:bg-red-500 text-red-500 hover:text-white font-bold py-3 rounded-xl flex items-center justify-center transition-colors">
                      <Trash2 size={18} className="mr-2" /> Удалить
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