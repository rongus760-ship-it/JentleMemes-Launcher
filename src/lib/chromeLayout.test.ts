import { describe, expect, it } from "vitest";
import {
  migrateChromeLayout,
  sidebarStyleFromLayout,
  toggleSidebarDensity,
  type ChromeLayout,
} from "./chromeLayout";

describe("migrateChromeLayout", () => {
  it("returns the stored layout when valid", () => {
    expect(migrateChromeLayout("top_tabs", null)).toBe("top_tabs");
    expect(migrateChromeLayout("sidebar_right_compact", null)).toBe("sidebar_right_compact");
  });

  it("falls back to sidebar_left_expanded for unknown value", () => {
    expect(migrateChromeLayout("unknown_xyz", "expanded")).toBe("sidebar_left_expanded");
    expect(migrateChromeLayout(null, "expanded")).toBe("sidebar_left_expanded");
    expect(migrateChromeLayout(undefined, null)).toBe("sidebar_left_expanded");
  });

  it("uses sidebar_left_compact when legacy sidebarStyle is compact", () => {
    expect(migrateChromeLayout("", "compact")).toBe("sidebar_left_compact");
    expect(migrateChromeLayout(null, "compact")).toBe("sidebar_left_compact");
  });
});

describe("sidebarStyleFromLayout", () => {
  it("maps layouts to expanded/compact", () => {
    const cases: [ChromeLayout, "expanded" | "compact"][] = [
      ["sidebar_left_expanded", "expanded"],
      ["sidebar_left_compact", "compact"],
      ["sidebar_right_expanded", "expanded"],
      ["sidebar_right_compact", "compact"],
      ["top_tabs", "expanded"],
      ["bottom_tabs", "expanded"],
    ];
    for (const [layout, expected] of cases) {
      expect(sidebarStyleFromLayout(layout)).toBe(expected);
    }
  });
});

describe("toggleSidebarDensity", () => {
  it("toggles between expanded and compact for sidebar layouts", () => {
    expect(toggleSidebarDensity("sidebar_left_expanded")).toBe("sidebar_left_compact");
    expect(toggleSidebarDensity("sidebar_left_compact")).toBe("sidebar_left_expanded");
    expect(toggleSidebarDensity("sidebar_right_expanded")).toBe("sidebar_right_compact");
    expect(toggleSidebarDensity("sidebar_right_compact")).toBe("sidebar_right_expanded");
  });

  it("leaves top_tabs and bottom_tabs unchanged", () => {
    expect(toggleSidebarDensity("top_tabs")).toBe("top_tabs");
    expect(toggleSidebarDensity("bottom_tabs")).toBe("bottom_tabs");
  });
});
