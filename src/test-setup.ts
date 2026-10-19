import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import "./i18n";

afterEach(() => {
  cleanup();
});

// jsdom ne fournit pas ResizeObserver (visionneuse, comparaison).
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
