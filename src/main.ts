import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { importPhoto, type AppError, type PhotoInfo } from "./api";
import {
  fit,
  PAGE_POINTS,
  toPercent,
  type MarginName,
  type PageSizeName,
} from "./layout";

type Photo = PhotoInfo & { path: string };

const photos: Photo[] = [];

const settings: { pageSize: PageSizeName; margin: MarginName } = {
  pageSize: "letter",
  margin: "small",
};

const seen = new Set<string>();

function byId(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (!element) throw new Error("missing element #" + id);
  return element;
}

const emptyScreen = byId("empty");
const photosScreen = byId("photos");
const grid = byId("grid");
const count = byId("count");
const errorList = byId("errors");

async function choosePhotos() {
  const picked = await open({
    multiple: true,
    filters: [
      {
        name: "Photos",
        extensions: ["jpg", "jpeg", "png", "webp", "heic", "heif"],
      },
    ],
  });
  if (!picked) return;
  await addPhotos(picked);
}

function fileNameOf(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

async function addPhotos(paths: string[]) {
  const sorted = [...paths].sort((a, b) =>
    fileNameOf(a).localeCompare(fileNameOf(b), undefined, { numeric: true }),
  );

  emptyScreen.hidden = true;
  photosScreen.hidden = false;

  for (const path of sorted) {
    if (seen.has(path)) continue; // already added or loading
    seen.add(path);

    try {
      const info = await importPhoto(path);
      const photo: Photo = { ...info, path };
      photos.push(photo);
      grid.appendChild(makeTile(photo, photos.length));
      count.textContent =
        photos.length + (photos.length === 1 ? " page" : " pages");
    } catch (e) {
      seen.delete(path); // so they can try this file again
      showError(fileNameOf(path), e as AppError);
    }
  }

  if (photos.length === 0) {
    emptyScreen.hidden = false;
    photosScreen.hidden = true;
  }
}

function makeTile(photo: Photo, number: number): HTMLElement {
  const tile = document.createElement("figure");

  const page = document.createElement("div");
  page.className = "page";
  const [pageW, pageH] = PAGE_POINTS[settings.pageSize];
  page.style.aspectRatio = pageW + " / " + pageH;

  const img = document.createElement("img");
  img.src = photo.thumb;
  img.alt = photo.name;
  const place = fit(
    settings.pageSize,
    settings.margin,
    photo.width,
    photo.height,
  );
  Object.assign(img.style, toPercent(place, settings.pageSize));

  const badge = document.createElement("span");
  badge.className = "badge";
  badge.textContent = String(number);

  page.append(img, badge);

  const caption = document.createElement("figcaption");
  caption.textContent = photo.name;

  tile.append(page, caption);
  return tile;
}

function showError(name: string, error: AppError) {
  const line = document.createElement("li");
  line.textContent = name + ": " + error.code + " (" + error.detail + ")";
  errorList.appendChild(line);
}

getCurrentWebview().onDragDropEvent((event) => {
  const type = event.payload.type;
  if (type === "enter") {
    document.body.classList.add("dragging");
  } else if (type === "leave") {
    document.body.classList.remove("dragging");
  } else if (type === "drop") {
    document.body.classList.remove("dragging");
    addPhotos(event.payload.paths);
  }
});

byId("choose").addEventListener("click", choosePhotos);
byId("add-more").addEventListener("click", choosePhotos);
