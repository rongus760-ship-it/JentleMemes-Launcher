import { defineConfig } from "vitest/config";

/**
 * Конфигурация vitest для юнит-тестов frontend-утилит.
 * Для запуска: `npm install -D vitest && npx vitest run`.
 * CI подхватит, как только vitest будет в devDependencies (см. Phase 2.5).
 */
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
    globals: false,
    watch: false,
  },
});
