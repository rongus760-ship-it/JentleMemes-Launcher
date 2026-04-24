import { openUrl } from "@tauri-apps/plugin-opener";

/** Совместимость с FriendsChatTab и прочими вызовами «открыть в системе». */
export async function openUrlInLauncher(url: string): Promise<void> {
  await openUrl(url);
}
