import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "../App";

describe("first launch", () => {
  it("offers Open Folder and explains that opening does not run commands", () => {
    render(<App />);
    expect(
      screen.getAllByRole("button", { name: "Open Folder" })[0],
    ).toBeVisible();
    expect(
      screen.getByText(/Opening a folder never runs its commands/),
    ).toBeVisible();
  });
});
