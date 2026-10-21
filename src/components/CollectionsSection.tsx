import { FolderPlus, ListFilter, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type Collection, sameFilter, useCollections } from "../store/collections";
import { isFiltering, useLibrary } from "../store/library";
import { confirmWarning } from "../ui/confirm";
import { IconButton } from "./IconButton";
import { ListRow, PanelNote, PanelSection } from "./PanelSection";

/** Saisie d'un nom dans le panneau : Entrée valide, Échap annule. */
function NameField({
  label,
  initial,
  onSubmit,
  onCancel,
}: {
  label: string;
  initial: string;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initial);
  return (
    <form
      className="collection-name"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim()) onSubmit(name.trim());
      }}
    >
      <input
        // Champ ouvert à la demande : le focus y va tout de suite.
        autoFocus
        aria-label={label}
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
        onBlur={onCancel}
      />
    </form>
  );
}

type Naming = { kind: Collection["kind"] } | { renameId: number } | null;

/** Collections manuelles et albums intelligents, dans le panneau de gauche. */
export function CollectionsSection({ scope }: { scope: string }) {
  const { t } = useTranslation();
  const collections = useCollections((s) => s.collections);
  const activeSmartId = useCollections((s) => s.activeSmartId);
  const libraryId = useLibrary((s) => s.libraryId);
  const media = useLibrary((s) => s.media);
  const filter = useLibrary((s) => s.filter);
  const albumId = useLibrary((s) => s.albumId);
  const collectionId = useLibrary((s) => s.collectionId);
  const trash = useLibrary((s) => s.trash);
  const showAlbums = useLibrary((s) => s.showAlbums);
  const selection = useLibrary((s) => s.targetIds().filter((id) => id > 0).length);
  const [naming, setNaming] = useState<Naming>(null);
  const st = useCollections.getState();

  // Les comptes suivent la grille : notes, corbeille, import.
  useEffect(() => {
    void useCollections.getState().load();
  }, [libraryId, media]);

  const isSelected = (c: Collection) =>
    !trash &&
    (c.kind === "manual"
      ? collectionId === c.id
      : activeSmartId === c.id && albumId === null && collectionId === null);
  const current = collections.find(isSelected);
  const filtering = isFiltering(filter) && !showAlbums;

  const remove = async (c: Collection) => {
    const ok = await confirmWarning(t(`collections.confirmDelete.${c.kind}`, { name: c.name }), {
      title: t("catalog.collections"),
      okLabel: t("collections.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await st.remove(c.id);
  };

  return (
    <PanelSection
      id={`${scope}.collections`}
      title={t("catalog.collections")}
      aside={
        <span className="panel-section-tools">
          <IconButton
            icon={FolderPlus}
            label={t("collections.newManual")}
            onClick={() => setNaming({ kind: "manual" })}
          />
          <IconButton
            icon={ListFilter}
            label={t("collections.newSmart")}
            disabled={!filtering}
            onClick={() => setNaming({ kind: "smart" })}
          />
        </span>
      }
    >
      {naming && "kind" in naming && (
        <NameField
          label={t(`collections.name.${naming.kind}`)}
          initial=""
          onCancel={() => setNaming(null)}
          onSubmit={(name) => {
            setNaming(null);
            void st.create(name, naming.kind);
          }}
        />
      )}
      {collections.length === 0 && !naming && <PanelNote>{t("catalog.noCollections")}</PanelNote>}
      {collections.map((c) =>
        naming && "renameId" in naming && naming.renameId === c.id ? (
          <NameField
            key={c.id}
            label={t("collections.rename")}
            initial={c.name}
            onCancel={() => setNaming(null)}
            onSubmit={(name) => {
              setNaming(null);
              void st.rename(c.id, name);
            }}
          />
        ) : (
          <div key={c.id} className="collection-row" data-kind={c.kind}>
            <ListRow
              label={c.name}
              hint={c.kind === "smart" ? t("collections.smart") : undefined}
              count={c.count}
              selected={isSelected(c)}
              onClick={() => void st.open(c)}
            />
            {c.kind === "manual" && selection > 0 && collectionId !== c.id && (
              <IconButton
                icon={Plus}
                label={t("collections.addSelection", { count: selection, name: c.name })}
                onClick={() => void st.addSelection(c.id)}
              />
            )}
          </div>
        ),
      )}
      {current && (
        <div className="collection-actions">
          <button
            type="button"
            className="link-button"
            onClick={() => setNaming({ renameId: current.id })}
          >
            {t("collections.rename")}
          </button>
          {current.kind === "smart" && !sameFilter(filter, current.filter ?? {}) && (
            <button
              type="button"
              className="link-button"
              onClick={() => void st.updateFilter(current.id)}
            >
              {t("collections.updateFilter")}
            </button>
          )}
          {current.kind === "manual" && selection > 0 && (
            <button type="button" className="link-button" onClick={() => void st.removeSelection()}>
              {t("collections.removeSelection", { count: selection })}
            </button>
          )}
          <button type="button" className="link-button" onClick={() => void remove(current)}>
            {t("collections.delete")}
          </button>
        </div>
      )}
    </PanelSection>
  );
}
