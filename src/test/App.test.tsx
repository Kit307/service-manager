import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "../App";

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
  expect(screen.getByText("0.6")).toBeVisible();
  expect(screen.getByRole("button", { name: "Import profile" })).toBeVisible();
});
