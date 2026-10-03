import { describe, expect, it } from "vitest";
import { cameraOf, viewOf } from "./camera";

describe("tree camera", () => {
  it("comes back to the same place", () => {
    const camera = { x: -340, y: 120, zoom: 0.6 };
    const size = { w: 1200, h: 800 };
    const centre = { x: 500, y: 300 };
    expect(cameraOf(viewOf(camera, size, centre), size, centre, 0.06, 2)).toEqual(camera);
  });

  it("keeps the centre person where they were when the layout or the window changed", () => {
    const view = viewOf({ x: 100, y: 50, zoom: 1 }, { w: 1000, h: 600 }, { x: 200, y: 100 });
    // The card moved 300 to the right (siblings unfolded) and the window is wider.
    const camera = cameraOf(view, { w: 1400, h: 600 }, { x: 500, y: 100 }, 0.06, 2);
    // The middle of the screen shows the same spot next to the card as before.
    expect((700 - camera.x) / camera.zoom - 500).toBeCloseTo(view.x);
    expect((300 - camera.y) / camera.zoom - 100).toBeCloseTo(view.y);
  });

  it("keeps the zoom within the canvas's limits", () => {
    expect(cameraOf({ zoom: 5, x: 0, y: 0 }, { w: 100, h: 100 }, { x: 0, y: 0 }, 0.06, 2).zoom).toBe(2);
    expect(cameraOf({ zoom: 0.001, x: 0, y: 0 }, { w: 100, h: 100 }, { x: 0, y: 0 }, 0.005, 1.6).zoom).toBe(0.005);
  });
});
