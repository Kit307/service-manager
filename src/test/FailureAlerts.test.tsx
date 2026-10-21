import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { FailureAlerts } from "../components/FailureAlerts";
import type { Project, ServiceState } from "../lib/types";
const project = (state: ServiceState, runId = "run-1"): Project => ({ id: "fixture", name: "Fixture", path: "/fixture", revision: "r1", trusted: [], config: { schemaVersion: 1, services: [{ id: "one", name: "One", cwd: ".", type: "command", command: "fixture" }] }, runtime: { one: { state, runId, health: "notConfigured" } } });
it("ignores baseline failures and manual stops, alerts once per failed run and opens logs", async () => {
  const select = vi.fn();
  const view = render(<FailureAlerts projects={[project("failed", "old")]} onService={select} />);
  expect(screen.queryByRole("button", { name: "View logs" })).not.toBeInTheDocument();
  view.rerender(<FailureAlerts projects={[project("running")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("stopped")]} onService={select} />);
  expect(screen.queryByRole("button", { name: "View logs" })).not.toBeInTheDocument();
  view.rerender(<FailureAlerts projects={[project("running", "new")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("failed", "new")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("failed", "new")]} onService={select} />);
  expect(screen.getAllByRole("button", { name: "View logs" })).toHaveLength(1);
  await userEvent.click(screen.getByRole("button", { name: "View logs" }));
  expect(select).toHaveBeenCalledWith({ projectId: "fixture", serviceId: "one" });
  await userEvent.click(screen.getByRole("button", { name: "Dismiss failure for One" }));
  expect(screen.queryByRole("button", { name: "View logs" })).not.toBeInTheDocument();
});

it("caps the center at 20 and dismisses all without a stuck count", async () => {
  const view = render(<FailureAlerts projects={[project("running")]} onService={vi.fn()} />);
  for (let index = 0; index < 23; index++) {
    view.rerender(<FailureAlerts projects={[project("running", `run-${index}`)]} onService={vi.fn()} />);
    view.rerender(<FailureAlerts projects={[project("failed", `run-${index}`)]} onService={vi.fn()} />);
  }
  expect(screen.getAllByRole("button", { name: "View logs" })).toHaveLength(20);
  await userEvent.click(screen.getByRole("button", { name: "Dismiss all" }));
  expect(screen.queryByRole("complementary", { name: "Service failure alerts" })).not.toBeInTheDocument();
});
it("does not alert when an explicitly stopping run fails", () => {
  const view = render(<FailureAlerts projects={[project("stopping")]} onService={vi.fn()} />);
  view.rerender(<FailureAlerts projects={[project("failed")]} onService={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "View logs" })).not.toBeInTheDocument();
});

it("does not resurface a dismissed failure when the same run transitions again", async () => {
  const select = vi.fn();
  const view = render(<FailureAlerts projects={[project("running", "repeat-run")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("failed", "repeat-run")]} onService={select} />);
  await userEvent.click(screen.getByRole("button", { name: "Dismiss failure for One" }));
  view.rerender(<FailureAlerts projects={[project("running", "repeat-run")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("failed", "repeat-run")]} onService={select} />);
  expect(screen.queryByRole("button", { name: "View logs" })).not.toBeInTheDocument();
  view.rerender(<FailureAlerts projects={[project("running", "next-run")]} onService={select} />);
  view.rerender(<FailureAlerts projects={[project("failed", "next-run")]} onService={select} />);
  expect(screen.getByRole("button", { name: "View logs" })).toBeVisible();
});
