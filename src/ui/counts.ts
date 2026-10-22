import type { TFunction } from "i18next";

/** « 12 photos », « 3 vidéos », « 12 photos et 3 vidéos ». */
export function mediaCount(t: TFunction, photos: number, videos: number): string {
  if (videos === 0) return t("media.photos", { count: photos });
  if (photos === 0) return t("media.videos", { count: videos });
  return t("media.both", {
    photos: t("media.photos", { count: photos }),
    videos: t("media.videos", { count: videos }),
  });
}
