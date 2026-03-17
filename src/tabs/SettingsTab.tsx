import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Save, Cpu, Terminal, FolderOpen, Layers, FlaskConical } from "lucide-react";
import { showToast } from "../App";

export default function SettingsTab() {
  const [settings, setSettings] = useState<Record<string, any>>({
    ram_mb: 4096,
    jvm_args: "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions",
    wrapper: "",
    close_on_launch: false,
    custom_java_path: "",
    download_dependencies: true,
    hybrid_provider_enabled: false,
    mod_provider: "modrinth",
    curseforge_api_key: "",
  });
  
  const [maxRam, setMaxRam] = useState(8192);

  useEffect(() => {
    invoke("load_settings").then((data: any) => setSettings(data));
    invoke("get_system_ram").then((ram: any) => { if (ram && ram > 1024) setMaxRam(ram); }).catch(console.error);
  },[]);

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      showToast("Настройки успешно сохранены!");
    } catch (e) { showToast(`Ошибка: ${e}`); }
  }

  return (
    <div className="flex flex-col items-center w-full max-w-4xl mx-auto h-full animate-in fade-in duration-300">
      <h2 className="text-3xl font-bold text-jm-accent-light mb-8 w-full text-left">Настройки лаунчера</h2>
      
      <div className="w-full bg-jm-card p-8 rounded-3xl border border-white/10 shadow-xl space-y-8">
        
        <div>
          <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2"><Cpu size={20}/> Выделение ОЗУ (RAM)</h3>
          <p className="text-sm text-gray-400 mb-4">Сколько оперативной памяти может использовать Minecraft. Выбрано: <strong className="text-jm-accent">{settings.ram_mb} MB</strong></p>
          <input type="range" min="1024" max={maxRam} step="512" value={settings.ram_mb} onChange={(e) => setSettings({...settings, ram_mb: parseInt(e.target.value)})} className="w-full accent-jm-accent cursor-pointer" />
          <div className="flex justify-between text-xs text-gray-500 mt-2 font-bold"><span>1 GB</span><span>MAX ({Math.round(maxRam / 1024)} GB)</span></div>
        </div>

        <div>
          <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2"><FolderOpen size={20}/> Путь к Java (Опционально)</h3>
          <p className="text-sm text-gray-400 mb-4">Оставьте пустым, чтобы лаунчер сам нашел нужную версию (8, 17, 21).</p>
          <input type="text" placeholder="/usr/lib/jvm/java-21-openjdk/bin/java" value={settings.custom_java_path} onChange={(e) => setSettings({...settings, custom_java_path: e.target.value})} className="w-full bg-[#0b110b] border border-white/10 rounded-xl px-4 py-3 text-white focus:border-jm-accent outline-none transition-colors" />
        </div>

        <div>
          <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2"><Terminal size={20}/> Аргументы JVM</h3>
          <p className="text-sm text-gray-400 mb-4">Продвинутые параметры запуска (например, флаги Aikar).</p>
          <input type="text" value={settings.jvm_args} onChange={(e) => setSettings({...settings, jvm_args: e.target.value})} className="w-full bg-[#0b110b] border border-white/10 rounded-xl px-4 py-3 text-white focus:border-jm-accent outline-none transition-colors font-mono text-sm" />
        </div>

        <div>
          <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2"><Layers size={20}/> Wrapper (Обертка)</h3>
          <p className="text-sm text-gray-400 mb-4">Команда, через которую запустится игра (например, mangohud или gamemoderun).</p>
          <input type="text" placeholder="mangohud" value={settings.wrapper} onChange={(e) => setSettings({...settings, wrapper: e.target.value})} className="w-full bg-[#0b110b] border border-white/10 rounded-xl px-4 py-3 text-white focus:border-jm-accent outline-none transition-colors font-mono text-sm" />
        </div>

        <label className="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5">
          <input type="checkbox" checked={settings.download_dependencies ?? true} onChange={(e) => setSettings({...settings, download_dependencies: e.target.checked})} className="w-5 h-5 accent-jm-accent cursor-pointer" />
          <span className="text-white font-bold">Скачивать зависимости модов автоматически</span>
        </label>
        <p className="text-xs text-gray-500 -mt-4">Если выключено, при установке мода не будут подтягиваться зависимости.</p>

        <div className="pt-6 border-t border-white/10">
          <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2"><FlaskConical size={20}/> Экспериментальные настройки</h3>
          <label className="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5 mt-2">
            <input type="checkbox" checked={!!settings.hybrid_provider_enabled} onChange={(e) => setSettings({...settings, hybrid_provider_enabled: e.target.checked})} className="w-5 h-5 accent-jm-accent cursor-pointer" />
            <span className="text-white font-bold">Гибридный режим браузера модов (Modrinth + CurseForge)</span>
          </label>
          <p className="text-xs text-gray-500 mt-2">Включите, чтобы в браузере модов появился выбор «Гибрид» и объединённый поиск без дубликатов.</p>
        </div>

        <div className="pt-6 border-t border-white/10 flex items-center justify-end">
          <button onClick={handleSave} className="bg-jm-accent hover:bg-jm-accent-light text-black font-bold px-8 py-3 rounded-xl flex items-center gap-2 transition-transform hover:scale-105 shadow-lg">
            <Save size={20}/> Сохранить настройки
          </button>
        </div>

      </div>
    </div>
  );
}