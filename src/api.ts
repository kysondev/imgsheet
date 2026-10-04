import { invoke } from "@tauri-apps/api/core";

export type PhotoInfo = {
  name: string;
  width: number;
  height: number;
  thumb: string;
};

export type AppError = {
  code: string;
  file: string | null;
  detail: string;
};

export function importPhoto(path: string): Promise<PhotoInfo> {
  return invoke<PhotoInfo>("import_photo", { path });
}
