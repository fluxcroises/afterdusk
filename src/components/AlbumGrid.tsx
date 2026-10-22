import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { type AlbumSummary, useLibrary, visibleAlbums } from "../store/library";
import { Thumbnail } from "./Thumbnail";
import { albumDates } from "../ui/format";

function AlbumCard({ album }: { album: AlbumSummary }) {
  const { t, i18n } = useTranslation();
  const version = useLibrary((s) =>
    album.coverMediaId === null ? 0 : (s.versions[album.coverMediaId] ?? 0),
  );
  const showAlbum = useLibrary((s) => s.showAlbum);
  const dates = albumDates(album, i18n.language);
  return (
    <button type="button" className="album-card" onClick={() => void showAlbum(album.id)}>
      <span className="album-card-cover">
        <Thumbnail
          src={album.coverMediaId === null ? null : previewUrl(album.coverMediaId, 256, version)}
        />
      </span>
      <span className="album-card-name">{album.name}</span>
      <span className="album-card-meta">
        {[dates, album.clientName, album.place, t("toolbar.photoCount", { count: album.count })]
          .filter(Boolean)
          .join(" · ")}
      </span>
    </button>
  );
}

/** Bibliothèque, vue Albums : les couvertures, triées par date, nom ou lieu. */
export function AlbumGrid() {
  const { t } = useTranslation();
  const albums = useLibrary((s) => s.albums);
  const query = useLibrary((s) => s.albumQuery);
  const sort = useLibrary((s) => s.albumSort);
  const shown = visibleAlbums(albums, query, sort);
  if (shown.length === 0) {
    return <p className="panel-note album-grid-empty">{t("filter.noAlbum")}</p>;
  }
  return (
    <div className="album-grid">
      {shown.map((a) => (
        <AlbumCard key={a.id} album={a} />
      ))}
    </div>
  );
}
