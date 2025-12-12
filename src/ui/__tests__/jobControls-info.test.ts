import { describe, it, expect, beforeEach, vi } from "vitest";
import { initJobControls } from "../jobControls";

vi.mock("../../lib/bridge", () => ({
  bridge: {
    invoke: vi.fn().mockResolvedValue(undefined),
  },
}));

function setupDom() {
  document.body.innerHTML = `
    <div>
      <select id="job-type-select"></select>
      <select id="max-concurrent-select"></select>
      <button id="job-type-info"></button>
      <div id="job-type-helper" class="info-popover"></div>
    </div>
  `;
}

describe("Job controls info helper", () => {
  beforeEach(() => {
    setupDom();
    initJobControls();
  });

  it("shows helper on click and hides on outside click", () => {
    const infoButton = document.getElementById("job-type-info") as HTMLButtonElement;
    const helper = document.getElementById("job-type-helper") as HTMLDivElement;
    expect(helper.classList.contains("visible")).toBe(false);

    infoButton.click();
    expect(helper.classList.contains("visible")).toBe(true);
    expect(infoButton.getAttribute("aria-expanded")).toBe("true");

    document.body.click();
    expect(helper.classList.contains("visible")).toBe(false);
    expect(infoButton.getAttribute("aria-expanded")).toBe("false");
  });
});
