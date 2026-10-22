import { Pencil, UserPlus } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type Client,
  type ClientAlbum,
  type ClientInput,
  sessionLabel,
  useClients,
} from "../store/clients";
import { useLibrary } from "../store/library";
import { confirmWarning } from "../ui/confirm";
import { albumDates } from "../ui/format";
import { IconButton } from "./IconButton";
import { ListRow, PanelNote, PanelSection } from "./PanelSection";

/** Clients, dans le panneau de gauche : un clic montre les photos de leurs albums. */
export function ClientsSection({ scope }: { scope: string }) {
  const { t } = useTranslation();
  const clients = useClients((s) => s.clients);
  const clientId = useLibrary((s) => s.filter.clientId);
  const albums = useLibrary((s) => s.albums);

  useEffect(() => {
    void useClients.getState().load();
  }, [albums]);

  return (
    <PanelSection
      id={`${scope}.clients`}
      title={t("clients.title")}
      aside={
        <span className="panel-section-tools">
          <IconButton
            icon={UserPlus}
            label={t("clients.new")}
            onClick={() => useClients.getState().openSheet({ id: null, attachTo: null })}
          />
        </span>
      }
    >
      {clients.length === 0 && <PanelNote>{t("clients.none")}</PanelNote>}
      {clients.map((c) => (
        <div key={c.id} className="collection-row">
          <ListRow
            label={c.name}
            hint={t("clients.albumCount", { count: c.albumCount })}
            selected={clientId === c.id}
            onClick={() =>
              void useLibrary.getState().setFilter({
                clientId: clientId === c.id ? undefined : c.id,
              })
            }
          />
          <IconButton
            icon={Pencil}
            label={t("clients.edit", { name: c.name })}
            onClick={() => useClients.getState().openSheet({ id: c.id })}
          />
        </div>
      ))}
    </PanelSection>
  );
}

const EMPTY: ClientInput = { name: "", email: "", phone: "", notes: "" };

/** Fiche client : coordonnées, notes, albums, suppression complète. */
export function ClientDialog() {
  const sheet = useClients((s) => s.sheet);
  const client = useClients((s) =>
    s.sheet?.id != null ? s.clients.find((c) => c.id === s.sheet?.id) : undefined,
  );
  if (!sheet) return null;
  // Une clé par fiche : le formulaire repart de la fiche ouverte.
  return <ClientForm key={client?.id ?? "new"} client={client} />;
}

function ClientForm({ client }: { client: Client | undefined }) {
  const { t, i18n } = useTranslation();
  const sessionTypes = useClients((s) => s.sessionTypes);
  const [form, setForm] = useState<ClientInput>(() =>
    client
      ? {
          name: client.name,
          email: client.email ?? "",
          phone: client.phone ?? "",
          notes: client.notes ?? "",
        }
      : EMPTY,
  );
  const [albums, setAlbums] = useState<ClientAlbum[]>([]);

  useEffect(() => {
    if (client) void useClients.getState().albumsOf(client.id).then(setAlbums);
    const escape = (e: KeyboardEvent) => e.key === "Escape" && useClients.getState().closeSheet();
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [client]);

  const st = useClients.getState();
  const close = () => st.closeSheet();
  const field = (key: keyof ClientInput) => ({
    value: form[key],
    onChange: (e: { currentTarget: { value: string } }) =>
      setForm({ ...form, [key]: e.currentTarget.value }),
  });

  const remove = async () => {
    if (!client) return;
    const ok = await confirmWarning(
      t("clients.confirmDelete", { name: client.name, count: client.albumCount }),
      {
        title: t("clients.title"),
        okLabel: t("clients.delete"),
        cancelLabel: t("trash.cancel"),
      },
    );
    if (ok) await st.remove(client.id);
  };

  const openAlbum = (a: ClientAlbum) => {
    close();
    const library = useLibrary.getState();
    if (a.libraryId === library.libraryId) void library.showAlbum(a.id);
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        className="copy-dialog client-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="client-dialog-title"
        onSubmit={(e) => {
          e.preventDefault();
          if (form.name.trim()) void st.save(form);
        }}
      >
        <h2 id="client-dialog-title" className="copy-title">
          {client ? client.name : t("clients.new")}
        </h2>
        <label className="field-row">
          <span className="field-row-label">{t("clients.name")}</span>
          <input type="text" required autoFocus maxLength={120} {...field("name")} />
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("clients.email")}</span>
          <input type="email" maxLength={200} {...field("email")} />
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("clients.phone")}</span>
          <input type="tel" maxLength={40} {...field("phone")} />
        </label>
        <label className="field-row client-notes">
          <span className="field-row-label">{t("clients.notes")}</span>
          <textarea rows={3} maxLength={2000} {...field("notes")} />
        </label>
        {client && (
          <div className="client-albums">
            <span className="field-row-label">{t("clients.albums")}</span>
            {albums.length === 0 ? (
              <PanelNote>{t("clients.noAlbum")}</PanelNote>
            ) : (
              <ul>
                {albums.map((a) => {
                  const type = sessionTypes.find((s) => s.id === a.sessionTypeId);
                  const here = a.libraryId === useLibrary.getState().libraryId;
                  return (
                    <li key={a.id}>
                      <button
                        type="button"
                        className="link-button"
                        disabled={!here}
                        onClick={() => openAlbum(a)}
                      >
                        {a.name}
                      </button>
                      <span className="panel-note">
                        {[
                          albumDates({ startDate: a.startDate, endDate: null }, i18n.language),
                          sessionLabel(t, type),
                          here ? null : a.libraryName,
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </span>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        )}
        <p className="panel-note">{t("clients.privacy")}</p>
        <div className="develop-crop-actions">
          {client && (
            <button
              type="button"
              className="button button-small button-danger"
              onClick={() => void remove()}
            >
              {t("clients.delete")}
            </button>
          )}
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={close}>
            {t("trash.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={!form.name.trim()}
          >
            {t("clients.save")}
          </button>
        </div>
      </form>
    </div>
  );
}

/** Album ouvert : son client et son type de séance. */
export function AlbumClientSection({ scope }: { scope: string }) {
  const { t } = useTranslation();
  const album = useLibrary((s) => s.albums.find((a) => a.id === s.albumId));
  const clients = useClients((s) => s.clients);
  const sessionTypes = useClients((s) => s.sessionTypes);

  useEffect(() => {
    void useClients.getState().load();
  }, []);

  if (!album) return null;
  const st = useClients.getState();
  return (
    <PanelSection id={`${scope}.albumClient`} title={t("clients.albumSection")}>
      <label className="field-row">
        <span className="field-row-label">{t("clients.client")}</span>
        <select
          value={album.clientId ?? ""}
          onChange={(e) => {
            const v = e.currentTarget.value;
            if (v === "new") {
              st.openSheet({ id: null, attachTo: album.id });
              return;
            }
            void st.setAlbum(album.id, v ? Number(v) : null, album.sessionTypeId);
          }}
        >
          <option value="">{t("clients.noClient")}</option>
          {clients.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
          <option value="new">{t("clients.newOption")}</option>
        </select>
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("sessions.type")}</span>
        <select
          value={album.sessionTypeId ?? ""}
          onChange={(e) => {
            const v = e.currentTarget.value;
            void st.setAlbum(album.id, album.clientId, v ? Number(v) : null);
          }}
        >
          <option value="">{t("sessions.none")}</option>
          {sessionTypes.map((s) => (
            <option key={s.id} value={s.id}>
              {sessionLabel(t, s)}
            </option>
          ))}
        </select>
      </label>
    </PanelSection>
  );
}
