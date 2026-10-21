import {
  type GeoJSONSource,
  LngLatBounds,
  Map as MapLibre,
  Marker,
  setWorkerUrl,
  type StyleSpecification,
} from "maplibre-gl";
import type { FeatureCollection, Point } from "geojson";
import "maplibre-gl/dist/maplibre-gl.css";
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url";
import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { useLibrary } from "../store/library";
import { type AlbumPoint, useMap } from "../store/map";
import { areaOf, wrapLongitude } from "../ui/geo";

setWorkerUrl(workerUrl);

/** Seul fond en ligne permis (décision 0008, CLAUDE.md). */
const ONLINE_STYLE = "https://tiles.openfreemap.org/styles/dark";
/** En deçà, les photos apparaissent une à une. */
const PHOTOS_ZOOM = 10;

function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Fond hors ligne : les frontières Natural Earth embarquées. */
async function offlineStyle(): Promise<StyleSpecification> {
  const countries = (await import("../assets/geo/countries.json")).default;
  return {
    version: 8,
    sources: {
      countries: { type: "geojson", data: countries as FeatureCollection },
    },
    layers: [
      {
        id: "background",
        type: "background",
        paint: { "background-color": token("--ad-workspace") },
      },
      {
        id: "land",
        type: "fill",
        source: "countries",
        paint: { "fill-color": token("--ad-panel") },
      },
      {
        id: "borders",
        type: "line",
        source: "countries",
        paint: { "line-color": token("--ad-field-border"), "line-width": 0.6 },
      },
    ],
  };
}

function albumsData(albums: AlbumPoint[]): FeatureCollection {
  return {
    type: "FeatureCollection",
    features: albums.map((a) => ({
      type: "Feature",
      properties: { albumId: a.albumId },
      geometry: { type: "Point", coordinates: [a.longitude, a.latitude] },
    })),
  };
}

/** Sources et couches d'Afterdusk, posées sur le fond (refaites à chaque fond). */
function addOverlays(map: MapLibre) {
  const empty: FeatureCollection = { type: "FeatureCollection", features: [] };
  const accent = token("--ad-accent");
  map.addSource("tracks", { type: "geojson", data: empty });
  map.addLayer({
    id: "tracks",
    type: "line",
    source: "tracks",
    paint: { "line-color": accent, "line-width": 2, "line-opacity": 0.8 },
  });
  map.addSource("area", { type: "geojson", data: empty });
  map.addLayer({
    id: "area-fill",
    type: "fill",
    source: "area",
    paint: { "fill-color": accent, "fill-opacity": 0.12 },
  });
  map.addLayer({
    id: "area-line",
    type: "line",
    source: "area",
    paint: { "line-color": accent, "line-width": 1.5 },
  });
  map.addSource("photos", { type: "geojson", data: empty });
  map.addLayer({
    id: "photos",
    type: "circle",
    source: "photos",
    minzoom: PHOTOS_ZOOM,
    paint: {
      "circle-radius": 4,
      "circle-color": accent,
      "circle-stroke-color": token("--ad-workspace"),
      "circle-stroke-width": 1,
    },
  });
  map.addSource("albums", {
    type: "geojson",
    data: empty,
    cluster: true,
    clusterRadius: 60,
    clusterMaxZoom: 14,
  });
  // Invisible : les albums sont dessinés en vignettes HTML, mais la source
  // n'est découpée en grappes que si une couche s'en sert.
  map.addLayer({
    id: "albums",
    type: "circle",
    source: "albums",
    paint: { "circle-opacity": 0, "circle-radius": 1 },
  });
}

/** Ce que lisent les vignettes au moment où elles sont dessinées. */
interface MarkerContext {
  albums: AlbumPoint[];
  onOpenAlbum: (albumId: number) => void;
  t: TFunction;
}

/** Vignettes d'albums et pastilles de grappes, refaites au fil des déplacements. */
function drawMarkers(map: MapLibre, markers: Map<string, Marker>, ctx: MarkerContext) {
  if (!map.getSource("albums") || !map.isSourceLoaded("albums")) return;
  // Vue détaillée : les photos une à une remplacent les vignettes d'albums.
  if (map.getZoom() >= PHOTOS_ZOOM) {
    for (const marker of markers.values()) marker.remove();
    markers.clear();
    return;
  }
  const seen = new Set<string>();
  const { albums: all, onOpenAlbum: open, t: tr } = ctx;
  for (const f of map.querySourceFeatures("albums")) {
    const props = f.properties as Record<string, unknown>;
    const [lon, lat] = (f.geometry as Point).coordinates;
    const clusterId = props.cluster ? (props.cluster_id as number) : null;
    const key = clusterId !== null ? `c${clusterId}` : `a${props.albumId as number}`;
    if (seen.has(key)) continue;
    seen.add(key);
    if (markers.has(key)) continue;
    const el = document.createElement("button");
    el.type = "button";
    if (clusterId !== null) {
      const count = props.point_count as number;
      el.className = "map-cluster";
      el.textContent = String(count);
      el.setAttribute("aria-label", tr("map.cluster", { count }));
      el.addEventListener("click", (e) => {
        e.stopPropagation();
        const source = map.getSource<GeoJSONSource>("albums");
        void source
          ?.getClusterExpansionZoom(clusterId)
          .then((zoom) => map.easeTo({ center: [lon, lat], zoom }));
      });
    } else {
      const album = all.find((a) => a.albumId === props.albumId);
      if (!album) continue;
      el.className = "map-album";
      el.dataset.albumId = String(album.albumId);
      el.toggleAttribute("aria-pressed", album.albumId === useMap.getState().selectedAlbumId);
      el.setAttribute("aria-label", album.name);
      el.title = album.name;
      const src = previewUrl(album.coverMediaId, 256);
      if (src) {
        const img = document.createElement("img");
        img.src = src;
        img.alt = "";
        el.append(img);
      }
      el.addEventListener("click", (e) => {
        e.stopPropagation();
        void useMap.getState().selectAlbum(album.albumId);
      });
      el.addEventListener("dblclick", (e) => {
        e.stopPropagation();
        open(album.albumId);
      });
    }
    markers.set(key, new Marker({ element: el }).setLngLat([lon, lat]).addTo(map));
  }
  for (const [key, marker] of markers) {
    if (!seen.has(key)) {
      marker.remove();
      markers.delete(key);
    }
  }
}

/** Carte du monde : albums en vignettes groupées, photos et traces en zoomant. */
export function WorldMap({ onOpenAlbum }: { onOpenAlbum: (albumId: number) => void }) {
  const { t } = useTranslation();
  const container = useRef<HTMLDivElement>(null);
  const mapRef = useRef<MapLibre | null>(null);
  const markers = useRef(new Map<string, Marker>());
  const online = useMap((s) => s.online);
  const albums = useMap((s) => s.albums);
  const media = useMap((s) => s.media);
  const tracks = useMap((s) => s.tracks);
  const area = useMap((s) => s.area);
  const mode = useMap((s) => s.mode);
  const selectedAlbumId = useMap((s) => s.selectedAlbumId);
  const fitted = useRef(false);
  const [styleLoads, setStyleLoads] = useState(0);
  // Les gestionnaires de la carte lisent l'état courant, pas celui de leur création.
  const latest = useRef<MarkerContext>({ albums, onOpenAlbum, t });
  useEffect(() => {
    latest.current = { albums, onOpenAlbum, t };
  });

  // Création de la carte, et changement de fond.
  useEffect(() => {
    if (!container.current) return;
    let cancelled = false;
    let fellBack = false;
    const map = new MapLibre({
      container: container.current,
      style: { version: 8, sources: {}, layers: [] },
      center: [10, 30],
      zoom: 1.3,
      attributionControl: { compact: true },
      dragRotate: false,
      pitchWithRotate: false,
    });
    mapRef.current = map;
    const placed = markers.current;
    const loadOffline = async () => {
      const style = await offlineStyle();
      if (!cancelled) map.setStyle(style);
    };
    if (online) {
      map.setStyle(ONLINE_STYLE);
      // Sans réseau (ou tuiles refusées) : le fond hors ligne, une fois.
      map.on("error", () => {
        if (!fellBack && !map.isStyleLoaded()) {
          fellBack = true;
          void loadOffline();
        }
      });
    } else void loadOffline();
    map.on("style.load", () => {
      addOverlays(map);
      // Nouveau fond : les données y sont reposées.
      setStyleLoads((n) => n + 1);
    });
    const draw = () => drawMarkers(map, markers.current, latest.current);
    map.on("render", draw);
    map.on("click", "photos", (e) => {
      const albumId = e.features?.[0]?.properties?.albumId as number | undefined;
      if (albumId !== undefined) void useMap.getState().selectAlbum(albumId);
    });
    return () => {
      cancelled = true;
      for (const m of placed.values()) m.remove();
      placed.clear();
      map.remove();
      mapRef.current = null;
      fitted.current = false;
    };
  }, [online]);

  // Données : albums, photos, traces, zone.
  useEffect(() => {
    const map = mapRef.current;
    if (!map?.getSource("albums")) return;
    map.getSource<GeoJSONSource>("albums")?.setData(albumsData(albums));
    for (const m of markers.current.values()) m.remove();
    markers.current.clear();
    map.getSource<GeoJSONSource>("photos")?.setData({
      type: "FeatureCollection",
      features: media.map((p) => ({
        type: "Feature",
        properties: { albumId: p.albumId },
        geometry: { type: "Point", coordinates: [p.longitude, p.latitude] },
      })),
    });
    if (!fitted.current && albums.length > 0) {
      fitted.current = true;
      const bounds = new LngLatBounds();
      for (const a of albums) bounds.extend([a.longitude, a.latitude]);
      map.fitBounds(bounds, { padding: 80, maxZoom: 10, duration: 0 });
    }
  }, [albums, media, styleLoads]);

  useEffect(() => {
    mapRef.current?.getSource<GeoJSONSource>("tracks")?.setData({
      type: "FeatureCollection",
      features: tracks.map((line) => ({
        type: "Feature",
        properties: {},
        geometry: { type: "LineString", coordinates: line },
      })),
    });
  }, [tracks, styleLoads]);

  useEffect(() => {
    const source = mapRef.current?.getSource<GeoJSONSource>("area");
    if (!source) return;
    if (!area) {
      source.setData({ type: "FeatureCollection", features: [] });
      return;
    }
    const [w, s, e, n] = area;
    const east = e < w ? e + 360 : e;
    source.setData({
      type: "Feature",
      properties: {},
      geometry: {
        type: "Polygon",
        coordinates: [
          [
            [w, s],
            [east, s],
            [east, n],
            [w, n],
            [w, s],
          ],
        ],
      },
    });
  }, [area, styleLoads]);

  // Album choisi dans la liste : la carte s'y rend.
  useEffect(() => {
    const map = mapRef.current;
    const album = albums.find((a) => a.albumId === selectedAlbumId);
    if (map && album) {
      map.easeTo({ center: [album.longitude, album.latitude], zoom: Math.max(map.getZoom(), 8) });
    }
    for (const m of markers.current.values()) {
      const el = m.getElement();
      el.toggleAttribute("aria-pressed", el.dataset.albumId === String(selectedAlbumId));
    }
  }, [selectedAlbumId, albums]);

  // Outils : tracer une zone, placer la sélection.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const canvas = map.getCanvasContainer();
    canvas.style.cursor = mode === "browse" ? "" : "crosshair";
    if (mode === "place") {
      const onClick = (e: { lngLat: { lng: number; lat: number } }) => {
        void useMap.getState().place(e.lngLat.lat, wrapLongitude(e.lngLat.lng));
      };
      map.on("click", onClick);
      return () => {
        map.off("click", onClick);
        canvas.style.cursor = "";
      };
    }
    if (mode !== "area") return;
    map.dragPan.disable();
    let start: { x: number; y: number; lng: number; lat: number } | null = null;
    const box = document.createElement("div");
    box.className = "map-area-box";
    const onDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const at = map.unproject([x, y]);
      start = { x, y, lng: at.lng, lat: at.lat };
      Object.assign(box.style, { left: `${x}px`, top: `${y}px`, width: "0", height: "0" });
      canvas.append(box);
    };
    const onMove = (e: MouseEvent) => {
      if (!start) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      Object.assign(box.style, {
        left: `${Math.min(x, start.x)}px`,
        top: `${Math.min(y, start.y)}px`,
        width: `${Math.abs(x - start.x)}px`,
        height: `${Math.abs(y - start.y)}px`,
      });
    };
    const onUp = (e: MouseEvent) => {
      if (!start) return;
      const rect = canvas.getBoundingClientRect();
      const end = map.unproject([e.clientX - rect.left, e.clientY - rect.top]);
      box.remove();
      const moved =
        Math.abs(e.clientX - rect.left - start.x) + Math.abs(e.clientY - rect.top - start.y);
      if (moved > 4) useMap.getState().setArea(areaOf([start.lng, start.lat], [end.lng, end.lat]));
      start = null;
    };
    canvas.addEventListener("mousedown", onDown);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      canvas.removeEventListener("mousedown", onDown);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      box.remove();
      map.dragPan.enable();
      canvas.style.cursor = "";
    };
  }, [mode, online]);

  // La sélection de la Bibliothèque sert au placement : tenue à jour.
  useLibrary((s) => s.selectedIds.length);

  return <div ref={container} className="world-map" aria-label={t("map.label")} role="region" />;
}
