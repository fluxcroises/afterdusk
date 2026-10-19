import type { ReactElement } from "react";
import { useAppEvents } from "./appEvents";
import { PerfOverlay } from "./components/PerfOverlay";
import { StatusBar } from "./components/StatusBar";
import { TopBar } from "./components/TopBar";
import { CullStep } from "./steps/CullStep";
import { DeliverStep } from "./steps/DeliverStep";
import { DevelopStep } from "./steps/DevelopStep";
import { ImportStep } from "./steps/ImportStep";
import { useLayout } from "./store/layout";
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
  useAppEvents("main");
  const screen = useNavigation((s) => s.screen);
  const Current = SCREENS[screen];
  const dropping = useLayout((s) => s.dropping);
  const immersive = useLayout((s) => s.immersive);

  return (
    <div className="app" data-dropping={dropping || undefined}>
      {!immersive && <TopBar />}
      <Current />
      {!immersive && <StatusBar />}
      <PerfOverlay />
    </div>
  );
}
