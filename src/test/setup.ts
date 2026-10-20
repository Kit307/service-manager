import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
afterEach(cleanup);

// JSDOM omits pointer capture; native WebKit exercises real capture in the GUI smoke check.
vi.stubGlobal("PointerEvent", MouseEvent);
HTMLElement.prototype.setPointerCapture = vi.fn();
