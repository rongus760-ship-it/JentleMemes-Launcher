/** Пропсы вкладки «Сборки» из `App.svelte` */
export type LibraryTabProps = {
  initialInstanceId?: string;
  initialServerIp?: string;
  initialWorldName?: string;
  onInstanceOpened?: () => void;
  onServerLaunchConsumed?: () => void;
  onWorldLaunchConsumed?: () => void;
  busyInstanceId?: string | null;
  progress?: { task_name: string; downloaded: number; total: number; instance_id?: string };
};
