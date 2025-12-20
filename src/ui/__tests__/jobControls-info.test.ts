import { describe, it, expect, beforeEach, vi } from "vitest";
import { getJobType, initJobControls } from "../jobControls";

vi.mock("../../lib/bridge", () => ({
  bridge: {
    invoke: vi.fn().mockResolvedValue(undefined),
  },
}));

function setupDom() {
  document.body.innerHTML = `
    <div>
      <input type="checkbox" id="merge-mode-toggle" />
      <select id="max-concurrent-select"></select>
    </div>
  `;
}

describe("Job controls merge toggle", () => {
  beforeEach(() => {
    setupDom();
    initJobControls();
  });

  it("dispatches job-type change and reflects merge toggle state", () => {
    let fired = false;
    document.addEventListener("abb:job-type-changed", () => {
      fired = true;
    });
    const toggle = document.getElementById("merge-mode-toggle") as HTMLInputElement;
    toggle.checked = true;
    toggle.dispatchEvent(new Event("change"));
    expect(fired).toBe(true);
    expect(getJobType()).toBe("merge");
  });
});
