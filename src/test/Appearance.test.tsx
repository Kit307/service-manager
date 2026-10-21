import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "../App";
import { api } from "../lib/api";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return { ...original, api: { ...original.api, workspace: vi.fn() } };
});

beforeEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  document.documentElement.style.removeProperty("--background-opacity");
  vi.mocked(api.workspace).mockResolvedValue({
    projects: [],
    groups: [],
    ai: { enabled: false, endpoint: "", clients: [] },
    uncleanShutdown: false,
  });
});

it("changes only the background preference and restores it after reopening", async () => {
  const user = userEvent.setup();
  const first = render(<App />);
  const slider = screen.getByRole("slider", { name: "Background opacity" });
  expect(slider).toHaveValue("55");
  expect(slider).toHaveAttribute("step", "5");
  fireEvent.change(slider, { target: { value: "85" } });
  expect(slider).toHaveAttribute("aria-valuetext", "85%");
  expect(screen.getByText("85%")).toBeVisible();
  expect(localStorage.getItem("glass-background-opacity")).toBe("85");
  expect(document.documentElement.style.getPropertyValue("--background-opacity")).toBe("85%");
  expect(document.documentElement.style.opacity).toBe("");

  await user.click(screen.getByRole("button", { name: "Change appearance" }));
  expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  expect(slider).toHaveValue("85");
  first.unmount();
  render(<App />);
  expect(screen.getByRole("slider", { name: "Background opacity" })).toHaveValue("85");
});

it.each(["invalid", "NaN", "Infinity", "", " "])(
  "uses the safe default for malformed stored opacity %j",
  (stored) => {
    localStorage.setItem("glass-background-opacity", stored);
    render(<App />);
    expect(screen.getByRole("slider", { name: "Background opacity" })).toHaveValue("55");
    expect(localStorage.getItem("glass-background-opacity")).toBe("55");
  },
);

it.each([["-20", "30"], ["200", "100"]])(
  "clamps stored opacity %s to %s",
  (stored, expected) => {
    localStorage.setItem("glass-background-opacity", stored);
    render(<App />);
    expect(screen.getByRole("slider", { name: "Background opacity" })).toHaveValue(expected);
    expect(localStorage.getItem("glass-background-opacity")).toBe(expected);
  },
);

it("keeps the slider usable when opacity storage is unavailable", async () => {
  const getItem = Storage.prototype.getItem;
  const setItem = Storage.prototype.setItem;
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(function (this: Storage, key) {
    if (key === "glass-background-opacity") throw new Error("Storage unavailable");
    return getItem.call(this, key);
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(function (this: Storage, key, value) {
    if (key === "glass-background-opacity") throw new Error("Storage unavailable");
    return setItem.call(this, key, value);
  });
  render(<App />);
  const slider = screen.getByRole("slider", { name: "Background opacity" });
  expect(slider).toHaveValue("55");
  fireEvent.change(slider, { target: { value: "100" } });
  await waitFor(() => expect(slider).toHaveAttribute("aria-valuetext", "100%"));
});
