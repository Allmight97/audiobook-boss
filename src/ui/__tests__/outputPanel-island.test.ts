import { beforeEach, describe, expect, it } from "vitest";
import { initOutputPanel } from "../outputPanel";

describe("OutputPanel island mount", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="output-panel-root"></div>
      <input id="meta-title" value="" />
      <input id="meta-author" value="" />
      <input id="meta-narrator" value="" />
      <input id="meta-year" value="" />
      <input id="meta-genre" value="" />
      <textarea id="meta-description"></textarea>
      <input id="meta-series" value="" />
      <input id="meta-series-part" value="" />
      <input id="meta-subseries" value="" />
      <input id="meta-subseries-part" value="" />
      <div id="meta-series-part-warning" hidden></div>
      <div id="meta-subseries-part-warning" hidden></div>
    `;
  });

  it("mounts output directory controls and renders default preview text", () => {
    initOutputPanel();

    const preview = document.getElementById("output-preview-text");
    expect(preview).toBeTruthy();
    expect(preview?.textContent).toBe("Select output directory...");
    expect(document.getElementById("output-dir-browse")).toBeTruthy();
  });
});
