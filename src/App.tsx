import type { ReactElement } from "react";
import { Sidebar } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { CullStep } from "./steps/CullStep";
import { DeliverStep } from "./steps/DeliverStep";
import { DevelopStep } from "./steps/DevelopStep";
import { ImportStep } from "./steps/ImportStep";
import { type Screen, useNavigation } from "./store/navigation";
import { GearView } from "./views/GearView";
import { LibraryView } from "./views/LibraryView";
import { MapView } from "./views/MapView";
import { SettingsView } from "./views/SettingsView";
import "./App.css";

const SCREENS: Record<Screen, () => ReactElement> = {
  import: ImportStep,
  cull: CullStep,
  develop: DevelopStep,
  deliver: DeliverStep,
  library: LibraryView,
  map: MapView,
  gear: GearView,
  settings: SettingsView,
};

export default function App() {
  const screen = useNavigation((s) => s.screen);
  const Current = SCREENS[screen];

  return (
    <div className="app">
      <TopBar />
      <div className="app-body">
        <Sidebar />
        <main className="app-main">
          <Current />
        </main>
      </div>
    </div>
  );
}
