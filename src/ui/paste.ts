import { useLibrary } from "../store/library";

/** Photos qui reçoivent un collage : la sélection de la pellicule, ou la photo ouverte. */
export function pasteTargets(): number[] {
  const { selectedIds, selectedId } = useLibrary.getState();
  if (selectedIds.length > 1) return selectedIds;
  return selectedId === null ? [] : [selectedId];
}
