import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
const confirm = vi.fn<() => Promise<boolean>>();
vi.mock("../ui/confirm", () => ({ confirmWarning: () => confirm() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useClients } = await import("../store/clients");
const { useLibrary } = await import("../store/library");
const { AlbumClientSection, ClientDialog, ClientsSection } = await import("./ClientsPanels");
const { SessionTypesSettings } = await import("../settings/Pages");

const dupont = {
  id: 3,
  name: "Famille Dupont",
  email: "dupont@example.com",
  phone: null,
  notes: "Préfère le noir et blanc",
  albumCount: 2,
};
const types = [
  { id: 1, key: "wedding", name: null },
  { id: 12, key: null, name: "Drone" },
];
const album = {
  id: 9,
  name: "Église",
  folder: "2026/Église",
  startDate: "2026-06-13",
  endDate: "2026-06-13",
  count: 120,
  place: null,
  coverMediaId: null,
  coverChosen: false,
  clientId: null,
  clientName: null,
  sessionTypeId: 1,
};

function reply(cmd: string): unknown {
  switch (cmd) {
    case "clients_list":
      return [dupont];
    case "session_types_list":
      return types;
    case "client_albums":
      return [
        {
          id: 9,
          libraryId: 1,
          libraryName: "Photos",
          name: "Église",
          startDate: "2026-06-13",
          sessionTypeId: 1,
        },
      ];
    case "client_create":
      return 4;
    case "albums_list":
      return [album];
    default:
      return null;
  }
}

describe("Clients et séances", () => {
  beforeEach(async () => {
    invoke.mockReset().mockImplementation((cmd) => Promise.resolve(reply(cmd)));
    confirm.mockReset().mockResolvedValue(true);
    useLibrary.setState({ libraryId: 1, albums: [album], albumId: 9, filter: {} } as never);
    useClients.setState({ sheet: null });
    await act(() => useClients.getState().load());
  });

  it("liste les clients et ouvre une fiche avec ses albums", async () => {
    render(
      <>
        <ClientsSection scope="test" />
        <ClientDialog />
      </>,
    );
    expect(screen.getByText("Famille Dupont")).toBeInTheDocument();
    expect(screen.getByText("2 albums")).toBeInTheDocument();
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Fiche de Famille Dupont" })),
    );
    expect(screen.getByLabelText("Courriel")).toHaveValue("dupont@example.com");
    expect(screen.getByLabelText("Notes")).toHaveValue("Préfère le noir et blanc");
    expect(screen.getByRole("button", { name: "Église" })).toBeInTheDocument();
    expect(screen.getByText(/Mariage/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Téléphone"), { target: { value: "06 12 34 56 78" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Enregistrer" })));
    expect(invoke).toHaveBeenCalledWith("client_update", {
      id: 3,
      client: {
        name: "Famille Dupont",
        email: "dupont@example.com",
        phone: "06 12 34 56 78",
        notes: "Préfère le noir et blanc",
      },
    });
    expect(useClients.getState().sheet).toBeNull();
  });

  it("supprime un client après confirmation", async () => {
    useClients.setState({ sheet: { id: 3 } });
    render(<ClientDialog />);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Supprimer le client" })),
    );
    expect(confirm).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("client_delete", { id: 3 });
  });

  it("rattache un nouveau client à l'album ouvert, puis change la séance", async () => {
    render(
      <>
        <AlbumClientSection scope="test" />
        <ClientDialog />
      </>,
    );
    expect(screen.getByLabelText("Séance")).toHaveDisplayValue("Mariage");
    await act(async () =>
      fireEvent.change(screen.getByLabelText("Client"), { target: { value: "new" } }),
    );
    fireEvent.change(screen.getByLabelText("Nom"), { target: { value: "  Martin  " } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Enregistrer" })));
    expect(invoke).toHaveBeenCalledWith("client_create", {
      client: { name: "Martin", email: null, phone: null, notes: null },
    });
    expect(invoke).toHaveBeenCalledWith("album_set_client", {
      albumId: 9,
      clientId: 4,
      sessionTypeId: 1,
    });
    await act(async () =>
      fireEvent.change(screen.getByLabelText("Séance"), { target: { value: "12" } }),
    );
    expect(invoke).toHaveBeenLastCalledWith("albums_list", { libraryId: 1 });
    expect(invoke).toHaveBeenCalledWith("album_set_client", {
      albumId: 9,
      clientId: null,
      sessionTypeId: 12,
    });
  });

  it("modifie la liste des types de séance", async () => {
    render(<SessionTypesSettings />);
    await act(async () => {});
    const wedding = screen.getByLabelText("Renommer Mariage");
    fireEvent.change(wedding, { target: { value: "Mariage civil" } });
    await act(async () => fireEvent.blur(wedding));
    expect(invoke).toHaveBeenCalledWith("session_type_rename", { id: 1, name: "Mariage civil" });
    fireEvent.change(screen.getByLabelText("Nouveau type de séance"), {
      target: { value: "Animalier" },
    });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Ajouter" })));
    expect(invoke).toHaveBeenCalledWith("session_type_create", { name: "Animalier" });
    await act(async () => fireEvent.click(screen.getAllByRole("button", { name: "Retirer" })[1]));
    expect(invoke).toHaveBeenCalledWith("session_type_delete", { id: 12 });
  });
});
