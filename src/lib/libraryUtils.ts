/** Наигранное время в instance.json в секундах */
export function formatPlaytimeSeconds(sec: number): string {
  const s = Math.max(0, Math.floor(sec || 0));
  if (s < 60) return `${s} с`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} м`;
  const h = Math.floor(m / 60);
  const mm = m % 60;
  if (h < 24) return mm > 0 ? `${h} ч ${mm} м` : `${h} ч`;
  const d = Math.floor(h / 24);
  const hh = h % 24;
  return hh > 0 ? `${d} д ${hh} ч` : `${d} д`;
}
