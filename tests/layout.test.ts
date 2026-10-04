import assert from "node:assert/strict";
import {
  fit,
  MARGIN_POINTS,
  PAGE_POINTS,
  type MarginName,
  type PageSizeName,
} from "../src/layout.ts";

function close(a: number, b: number) {
  assert.ok(Math.abs(a - b) < 0.01, a + " is not close to " + b);
}

let p = fit("letter", "small", 3024, 4032);
close(p.h, 756);
close(p.w, 567);
close(p.x, 22.5);
close(p.y, 18);

p = fit("letter", "small", 4032, 3024);
close(p.w, 576);
close(p.h, 432);
close(p.x, 18);
close(p.y, 180);

const [a4w, a4h] = PAGE_POINTS.a4;
p = fit("a4", "none", 1000, 1000);
close(p.w, a4w);
close(p.h, a4w);
close(p.x, 0);
close(p.y, (a4h - a4w) / 2);

const sizes: PageSizeName[] = ["letter", "a4"];
const margins: MarginName[] = ["none", "small", "medium"];
const photos = [
  [4032, 3024],
  [3024, 4032],
  [4032, 2268],
  [1000, 4000],
  [4000, 1000],
  [50, 50],
  [1, 5000],
];
for (const size of sizes) {
  for (const margin of margins) {
    for (const [photoW, photoH] of photos) {
      const r = fit(size, margin, photoW, photoH);
      const [pageW, pageH] = PAGE_POINTS[size];
      const m = MARGIN_POINTS[margin];
      const tiny = 0.001;
      assert.ok(r.x >= m - tiny && r.y >= m - tiny);
      assert.ok(r.x + r.w <= pageW - m + tiny);
      assert.ok(r.y + r.h <= pageH - m + tiny);
      assert.ok(
        Math.abs(r.w / r.h - photoW / photoH) / (photoW / photoH) < 0.0001,
      );
    }
  }
}

console.log("layout tests passed");
