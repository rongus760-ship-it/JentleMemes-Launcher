import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { User, Mail, Key, MonitorSmartphone, Trash2, CheckCircle2 } from "lucide-react";

export default function AccountTab() {
  const[profiles, setProfiles] = useState<any>({ accounts:[], active_account_id: "", skin_presets: [] });
  const[authType, setAuthType] = useState("offline");
  const [status, setStatus] = useState("");

  const [offlineName, setOfflineName] = useState("");
  const[elyEmail, setElyEmail] = useState("");
  const[elyPass, setElyPass] = useState("");
  const[msCodeData, setMsCodeData] = useState<any>(null);

  useEffect(() => { invoke("load_profiles").then((data: any) => setProfiles(data)); },[]);

  async function saveAndEmit(newProfiles: any) {
    setProfiles(newProfiles);
    await invoke("save_profiles", { profiles: newProfiles });
    await emit("profiles_updated");
  }

  async function handleAddAccount(acc: any) {
    await saveAndEmit({ ...profiles, accounts: [...profiles.accounts, acc], active_account_id: acc.id });
  }

  async function handleDelete(id: string) {
    const newAccounts = profiles.accounts.filter((a: any) => a.id !== id);
    await saveAndEmit({ ...profiles, accounts: newAccounts, active_account_id: profiles.active_account_id === id ? (newAccounts.length > 0 ? newAccounts[0].id : "") : profiles.active_account_id });
  }

  async function handleSetActive(id: string) { await saveAndEmit({ ...profiles, active_account_id: id }); }

  async function handleOfflineLogin() {
    if (!offlineName.trim()) return setStatus("Введите никнейм!");
    setStatus("Создание оффлайн аккаунта...");
    try {
      const acc = await invoke("login_offline", { username: offlineName });
      await handleAddAccount(acc);
      setStatus("Аккаунт добавлен!");
      setOfflineName("");
    } catch (e) { setStatus(`Ошибка: ${e}`); }
  }

  async function handleElybyLogin() {
    if (!elyEmail || !elyPass) return setStatus("Введите email и пароль!");
    setStatus("Авторизация в Ely.by...");
    try {
      const acc = await invoke("login_elyby", { email: elyEmail, password: elyPass });
      await handleAddAccount(acc);
      setStatus("Успешный вход через Ely.by!");
      setElyEmail(""); setElyPass("");
    } catch (e) { setStatus(`Ошибка: ${e}`); }
  }

  async function handleMicrosoftInit() {
    setStatus("Связь с серверами Microsoft...");
    try {
      const data: any = await invoke("ms_init_device_code");
      setMsCodeData(data);
      setStatus("Ожидание авторизации в браузере... Перейдите по ссылке и введите код!");
      const acc = await invoke("ms_login_poll", { deviceCode: data.device_code, interval: data.interval });
      await handleAddAccount(acc);
      setStatus("Успешный вход через Microsoft!");
      setMsCodeData(null);
    } catch (e) { setStatus(`Ошибка: ${e}`); setMsCodeData(null); }
  }

  return (
    <div className="flex gap-8 w-full max-w-6xl mx-auto items-start">
      <div className="w-1/2 bg-jm-card p-6 rounded-2xl border border-white/10 shadow-xl">
        <h2 className="text-2xl font-bold text-jm-accent-light mb-6 flex items-center gap-2"><User /> Ваши аккаунты</h2>
        {profiles.accounts.length === 0 ? (
          <div className="text-gray-500 text-center py-10">Нет добавленных аккаунтов</div>
        ) : (
          <div className="space-y-3">
            {profiles.accounts.map((acc: any) => {
              const isActive = profiles.active_account_id === acc.id;
              let avatarUrl = `https://minotar.net/helm/${acc.username}/48.png`;
              if (acc.active_skin_id && profiles.skin_presets) {
                const skin = profiles.skin_presets.find((p: any) => p.id === acc.active_skin_id);
                if (skin) {
                  if (skin.skin_type === "local") avatarUrl = skin.skin_data;
                  else avatarUrl = `https://minotar.net/helm/${skin.skin_data || skin.username}/48.png`;
                }
            }
              return (
                <div key={acc.id} onClick={() => handleSetActive(acc.id)} className={`flex items-center justify-between p-4 rounded-xl border transition-all cursor-pointer ${isActive ? "bg-jm-accent/20 border-jm-accent shadow-[0_0_15px_rgba(134,168,134,0.1)]" : "bg-black/40 border-white/5 hover:border-jm-accent/30"}`}>
                  <div className="flex items-center gap-4">
                    <img src={avatarUrl} alt="avatar" className="w-12 h-12 rounded-lg object-cover" style={{ imageRendering: "pixelated" }} />
                    <div>
                      <div className="font-bold text-lg text-white flex items-center gap-2">{acc.username}{isActive && <CheckCircle2 size={16} className="text-jm-accent" />}</div>
                      <div className="text-xs text-gray-400 uppercase tracking-wider">{acc.acc_type}</div>
                    </div>
                  </div>
                  <button onClick={(e) => { e.stopPropagation(); handleDelete(acc.id); }} className="p-2 text-gray-500 hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors"><Trash2 size={18} /></button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="w-1/2 bg-jm-card p-6 rounded-2xl border border-white/10 shadow-xl">
        <h2 className="text-2xl font-bold text-jm-accent-light mb-6">Добавить аккаунт</h2>
        <div className="flex gap-2 mb-6 bg-black/30 p-1 rounded-xl border border-white/5">
          {["offline", "elyby", "microsoft"].map((type) => (
            <button key={type} onClick={() => { setAuthType(type); setStatus(""); }} className={`flex-1 py-2 rounded-lg text-sm font-bold transition-all ${authType === type ? "bg-jm-accent text-black shadow-md" : "text-gray-400 hover:text-white"}`}>
              {type === "offline" && "Пиратка"}{type === "elyby" && "Ely.by"}{type === "microsoft" && "Microsoft"}
            </button>
          ))}
        </div>
        {authType === "offline" && (
          <div className="space-y-4 animate-in fade-in">
            <div>
              <label className="text-sm text-gray-400 mb-1 block">Никнейм</label>
              <div className="relative"><User className="absolute left-3 top-3 text-gray-500" size={18} /><input type="text" value={offlineName} onChange={(e) => setOfflineName(e.target.value)} placeholder="Steve" className="w-full bg-[#0b110b] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none" /></div>
            </div>
            <button onClick={handleOfflineLogin} className="w-full bg-jm-accent hover:bg-jm-accent-light text-black font-bold py-3 rounded-xl transition-colors">Добавить пиратку</button>
          </div>
        )}
        {authType === "elyby" && (
          <div className="space-y-4 animate-in fade-in">
            <div><label className="text-sm text-gray-400 mb-1 block">Email или Ник</label><div className="relative"><Mail className="absolute left-3 top-3 text-gray-500" size={18} /><input type="text" value={elyEmail} onChange={(e) => setElyEmail(e.target.value)} className="w-full bg-[#0b110b] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none" /></div></div>
            <div><label className="text-sm text-gray-400 mb-1 block">Пароль</label><div className="relative"><Key className="absolute left-3 top-3 text-gray-500" size={18} /><input type="password" value={elyPass} onChange={(e) => setElyPass(e.target.value)} className="w-full bg-[#0b110b] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none" /></div></div>
            <button onClick={handleElybyLogin} className="w-full bg-[#0078D7] hover:bg-[#1f8ce1] text-white font-bold py-3 rounded-xl transition-colors">Войти через Ely.by</button>
          </div>
        )}
        {authType === "microsoft" && (
          <div className="space-y-4 animate-in fade-in text-center">
            {!msCodeData ? (
              <><MonitorSmartphone className="mx-auto text-gray-400 mb-4" size={48} /><p className="text-gray-400 text-sm mb-6">Безопасный вход через Device Code.</p><button onClick={handleMicrosoftInit} className="w-full bg-[#107C10] hover:bg-[#149614] text-white font-bold py-3 rounded-xl transition-colors">Получить код входа</button></>
            ) : (
              <div className="bg-black/40 p-6 rounded-xl border border-jm-accent/30"><p className="text-sm text-gray-400 mb-2">1. Откройте эту ссылку:</p><a href={msCodeData.verification_uri} target="_blank" rel="noreferrer" className="text-jm-accent-light font-bold text-lg hover:underline block mb-6">{msCodeData.verification_uri}</a><p className="text-sm text-gray-400 mb-2">2. Введите код:</p><div className="bg-black text-white text-3xl font-mono tracking-widest py-4 rounded-lg border border-white/10 mb-4">{msCodeData.user_code}</div><p className="text-xs text-jm-accent animate-pulse">⏳ Ожидание авторизации...</p></div>
            )}
          </div>
        )}
        {status && <div className="mt-6 text-sm text-center text-jm-accent-light bg-black/40 px-4 py-3 rounded-lg border border-jm-accent/20">{status}</div>}
      </div>
    </div>
  );
}