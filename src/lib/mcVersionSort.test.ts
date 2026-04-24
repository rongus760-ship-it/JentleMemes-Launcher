import { describe, expect, it } from "vitest";
import { sortMcVersionsDesc } from "./mcVersionSort";

describe("sortMcVersionsDesc", () => {
  it("sorts semver-like versions in descending order", () => {
    expect(sortMcVersionsDesc(["1.20.1", "1.21", "1.19.2", "1.21.5"])).toEqual([
      "1.21.5",
      "1.21",
      "1.20.1",
      "1.19.2",
    ]);
  });

  it("respects numeric ordering (1.19 > 1.2)", () => {
    expect(sortMcVersionsDesc(["1.2", "1.19", "1.3"])).toEqual(["1.19", "1.3", "1.2"]);
  });

  it("returns a new array without mutating the input", () => {
    const input = ["1.20", "1.21"];
    const output = sortMcVersionsDesc(input);
    expect(input).toEqual(["1.20", "1.21"]);
    expect(output).toEqual(["1.21", "1.20"]);
  });
});
