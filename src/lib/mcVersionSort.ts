/** Сортировка ID версий Minecraft: 1.20 выше 1.19, 1.19 выше 1.2 (не лексикографически). */
export function sortMcVersionsDesc(versions: string[]): string[] {
  return [...versions].sort((a, b) =>
    b.localeCompare(a, undefined, { numeric: true, sensitivity: "base" }),
  );
}
