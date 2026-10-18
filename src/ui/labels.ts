export const COLOR_LABELS = ["red", "yellow", "green", "blue", "purple"] as const;

export type ColorLabel = (typeof COLOR_LABELS)[number];
