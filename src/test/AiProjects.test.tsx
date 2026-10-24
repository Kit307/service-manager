import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { AiConnections } from "../components/AiConnections";
import { api } from "../lib/api";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return {
    ...original,
    api: { ...original.api, createClient: vi.fn() },
  };
});

it("shows existing profiles as full access without a folder grant picker", () => {
  render(<AiConnections workspace={{ projects: [], groups: [], ai: {
    enabled: true, endpoint: "", clients: [{id:"legacy",name:"Existing client",projects:[],projectRoots:[],control:true,edit:true}],
  }, uncleanShutdown:false }} onRefresh={async()=>{}} onError={vi.fn()} />);
  expect(screen.getByText("Existing client")).toBeVisible();
  expect(screen.getByText(/Full access · Create\/open projects/)).toBeVisible();
  expect(screen.queryByRole("button", {name:"Add allowed folder"})).not.toBeInTheDocument();
  expect(screen.getByText(/macOS privacy permissions still apply/)).toBeVisible();
});

it("creates a full-access token without choosing projects or roots", async () => {
  vi.mocked(api.createClient).mockResolvedValue({
    id: "fixture",
    token: "fixture-token",
    proxyPath: "/fixture/proxy.sh",
  });
  const user = userEvent.setup();
  render(
    <AiConnections
      workspace={{
        projects: [],
        groups: [],
        ai: { enabled: true, endpoint: "", clients: [] },
        uncleanShutdown: false,
      }}
      onRefresh={async () => {}}
      onError={vi.fn()}
    />,
  );
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  await user.click(
    screen.getByRole("button", { name: "Create profile & proxy" }),
  );
  expect(api.createClient).toHaveBeenLastCalledWith(
    "Codex",
    [],
    true,
    true,
  );
});
