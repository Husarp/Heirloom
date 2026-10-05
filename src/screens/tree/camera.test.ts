import { describe, expect, it } from "vitest";
import { cameraOf, unfoldView, viewOf } from "./camera";

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

  describe("a branch unfolded in Potomkowie", () => {
    const view = { left: 136, top: 110, right: 1476, bottom: 876 };
    const onScreen = (c: { x: number; y: number; zoom: number }, wx: number, wy: number) => ({ x: c.x + wx * c.zoom, y: c.y + wy * c.zoom });

    it("stays put when the person and their children are already in view", () => {
      const camera = { x: 300, y: 150, zoom: 1 };
      expect(unfoldView(camera, { x: 0, y: 0, r: 600, b: 252 }, { x: 102, y: 0 }, view)).toBe(camera);
    });

    it("moves by the least amount to bring the row in from the right", () => {
      // The person's card at screen x 1402, only 74 px of it showing; the row of children is 900 wide.
      const camera = { x: 1402, y: 300, zoom: 1 };
      const next = unfoldView(camera, { x: -300, y: 0, r: 600, b: 252 }, { x: 102, y: 0 }, view);
      expect(next.zoom).toBe(1);
      expect(next.y).toBe(300);
      expect(onScreen(next, 600, 0).x).toBe(view.right);
    });

    it("zooms out a little for a wider row, but not below full cards, then centres on the person", () => {
      const camera = { x: 1402, y: 300, zoom: 1 };
      const slightly = unfoldView(camera, { x: -400, y: 0, r: 1200, b: 252 }, { x: 102, y: 0 }, view);
      expect(slightly.zoom).toBeCloseTo(1340 / 1600);
      expect(onScreen(slightly, -400, 0).x).toBeGreaterThanOrEqual(view.left - 0.01);
      expect(onScreen(slightly, 1200, 0).x).toBeLessThanOrEqual(view.right + 0.01);
      const wide = unfoldView(camera, { x: -3000, y: 0, r: 3204, b: 252 }, { x: 102, y: 0 }, view);
      expect(wide.zoom).toBe(0.75);
      expect(onScreen(wide, 102, 0).x).toBe((view.left + view.right) / 2);
    });

    it("comes up when the row is below the screen", () => {
      const next = unfoldView({ x: 300, y: 700, zoom: 1 }, { x: 0, y: 0, r: 600, b: 252 }, { x: 102, y: 0 }, view);
      expect(onScreen(next, 0, 252).y).toBe(view.bottom);
    });
  });
});
