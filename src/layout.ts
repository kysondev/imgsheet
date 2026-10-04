export type PageSizeName = "letter" | "a4";
export type MarginName = "none" | "small" | "medium";

export const PAGE_POINTS: Record<PageSizeName, [number, number]> = {
  letter: [612, 792],
  a4: [595.276, 841.89],
};

export const MARGIN_POINTS: Record<MarginName, number> = {
  none: 0,
  small: 18,
  medium: 36,
};

export type Placement = { x: number; y: number; w: number; h: number };

export function fit(
  size: PageSizeName,
  margin: MarginName,
  photoW: number,
  photoH: number,
): Placement {
  const [pageW, pageH] = PAGE_POINTS[size];
  const m = MARGIN_POINTS[margin];
  const areaW = pageW - 2 * m;
  const areaH = pageH - 2 * m;

  const scale = Math.min(areaW / photoW, areaH / photoH);
  const w = photoW * scale;
  const h = photoH * scale;

  return {
    x: m + (areaW - w) / 2,
    y: m + (areaH - h) / 2,
    w,
    h,
  };
}

export function toPercent(place: Placement, size: PageSizeName) {
  const [pageW, pageH] = PAGE_POINTS[size];
  return {
    left: (place.x / pageW) * 100 + "%",
    top: ((pageH - place.y - place.h) / pageH) * 100 + "%",
    width: (place.w / pageW) * 100 + "%",
    height: (place.h / pageH) * 100 + "%",
  };
}
