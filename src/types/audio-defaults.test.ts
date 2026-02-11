import { describe, expect, it } from "vitest";
import { defaultEncoderSettings } from "./audio";

describe("defaultEncoderSettings", () => {
  it("defaults to auto encoder with VBR mode", () => {
    const defaults = defaultEncoderSettings();
    expect(defaults.encoderType).toBe("auto");
    expect(defaults.bitrateMode).toEqual({ mode: "vbr", value: 3 });
  });
});

