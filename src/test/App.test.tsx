import { render, screen } from "@testing-library/react";
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
