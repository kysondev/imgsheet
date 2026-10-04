import { open } from "@tauri-apps/plugin-dialog";
import { importPhoto, type AppError, type PhotoInfo } from "./api";

type Photo = PhotoInfo & { path: string };

const photos: Photo[] = [];

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
    if (photos.some((photo) => photo.path === path)) continue; // already added

    try {
      const info = await importPhoto(path);
      const photo: Photo = { ...info, path };
      photos.push(photo);
      grid.appendChild(makeTile(photo));
      count.textContent =
        photos.length + (photos.length === 1 ? " page" : " pages");
    } catch (e) {
      showError(fileNameOf(path), e as AppError);
    }
  }

  if (photos.length === 0) {
    emptyScreen.hidden = false;
    photosScreen.hidden = true;
  }
}

function makeTile(photo: Photo): HTMLElement {
  const tile = document.createElement("figure");

  const img = document.createElement("img");
  img.src = photo.thumb;
  img.alt = photo.name;

  const caption = document.createElement("figcaption");
  caption.textContent = photo.name + " · " + photo.width + "×" + photo.height;

  tile.append(img, caption);
  return tile;
}

function showError(name: string, error: AppError) {
  const line = document.createElement("li");
  line.textContent = name + ": " + error.code + " (" + error.detail + ")";
  errorList.appendChild(line);
}

byId("choose").addEventListener("click", choosePhotos);
byId("add-more").addEventListener("click", choosePhotos);
