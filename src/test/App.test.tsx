import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "../App";
import packageInfo from "../../package.json";

describe("first launch", () => {
  it("offers Add service and explains that adding does not run commands", () => {
    render(<App />);
    expect(
      screen.getAllByRole("button", { name: "Add service" })[0],
    ).toBeVisible();
    expect(
      screen.getByText(/Adding a folder never runs its commands/),
    ).toBeVisible();
  });
});

it("opens central VPN settings with legacy workspace data and preserves the version badge", async () => {
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Docker VPN" }));
  expect(screen.getByRole("heading", { name: "Shared Docker VPN" })).toBeVisible();
  expect(screen.getByText(packageInfo.version)).toBeVisible();
  expect(screen.getByRole("button", { name: "Import profile" })).toBeVisible();
});

it("exposes the native ops shell landmarks", () => {
  render(<App />);
  expect(document.querySelector(".app-shell")).toHaveClass("ops-shell");
  expect(screen.getByRole("complementary", { name: "Services" })).toHaveClass("ops-sidebar");
  expect(screen.getByRole("main")).toHaveClass("ops-workspace");
  expect(screen.getByRole("banner", { name: "Workspace context" })).toBeVisible();
});

it("opens Live Network from the sidebar and shows its breadcrumb", async () => {
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Live Network" }));
  expect(screen.getByRole("banner", { name: "Workspace context" })).toHaveTextContent("Live Network");
  expect(screen.getByRole("heading", { name: "Live Network" })).toBeVisible();
});
