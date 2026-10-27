import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { LogPanel, type LogViewState } from "../components/LogPanel";
import type { LogRecord } from "../lib/types";

const logs: LogRecord[] = [
  {
    seq: 1,
    runId: "fixture",
    timestamp: "2026-10-21T00:00:00Z",
    stream: "stdout",
    text: "INFO server ready",
    truncated: false,
  },
  {
    seq: 2,
    runId: "fixture",
    timestamp: "2026-10-21T00:00:01Z",
    stream: "stderr",
    text: "ERROR worker failed",
    truncated: false,
  },
];
const initial: LogViewState = { stream: "all", query: "", follow: true };
const props = { logs, active: true, gap: false, onClear: vi.fn() };

it("renders controlled filters and follow state from props and reports edits", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  const state: LogViewState = {
    stream: "stderr",
    query: "failed",
    follow: false,
  };
  const { rerender } = render(
    <LogPanel {...props} viewState={state} onViewStateChange={change} />,
  );
  expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue("failed");
  expect(screen.getByRole("button", { name: "stderr" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Follow" })).toHaveAttribute("aria-pressed", "false");
  expect(screen.getByText("ERROR worker failed")).toBeVisible();
  expect(screen.queryByText("INFO server ready")).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "Search logs" }), {
    target: { value: "worker" },
  });
  expect(change).toHaveBeenLastCalledWith({ ...state, query: "worker" });
  expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue("failed");
  await user.click(screen.getByRole("button", { name: "stdout" }));
  expect(change).toHaveBeenLastCalledWith({ ...state, stream: "stdout" });
  await user.click(screen.getByRole("button", { name: "Follow" }));
  expect(change).toHaveBeenLastCalledWith({ ...state, follow: true });

  rerender(<LogPanel {...props} viewState={initial} onViewStateChange={change} />);
  expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue("");
  expect(screen.getByRole("button", { name: "All output" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Following" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText("INFO server ready")).toBeVisible();
});

it("reports manual scrolling as a controlled follow change", () => {
  const change = vi.fn();
  render(<LogPanel {...props} viewState={initial} onViewStateChange={change} />);
  const viewport = screen.getByRole("region", { name: "Log output" });
  Object.defineProperties(viewport, {
    scrollHeight: { configurable: true, value: 1000 },
    clientHeight: { configurable: true, value: 200 },
  });
  fireEvent.scroll(viewport, { target: { scrollTop: 100 } });
  expect(change).toHaveBeenCalledWith({ ...initial, follow: false });
});

it("keeps query, filter and follow independent between controlled instances", async () => {
  function Panels() {
    const [left, setLeft] = useState(initial);
    const [right, setRight] = useState(initial);
    return <>
      <LogPanel {...props} label="API logs" viewState={left} onViewStateChange={setLeft} />
      <LogPanel {...props} label="Worker logs" viewState={right} onViewStateChange={setRight} />
    </>;
  }
  const user = userEvent.setup();
  render(<Panels />);
  const left = within(screen.getByRole("region", { name: "API logs" }));
  const right = within(screen.getByRole("region", { name: "Worker logs" }));
  await user.type(left.getByRole("textbox", { name: "Search logs" }), "server");
  await user.click(left.getByRole("button", { name: "stdout" }));
  await user.click(left.getByRole("button", { name: "Following" }));
  expect(left.queryByText("ERROR worker failed")).not.toBeInTheDocument();
  expect(left.getByRole("button", { name: "Follow" })).toBeVisible();
  expect(right.getByRole("textbox", { name: "Search logs" })).toHaveValue("");
  expect(right.getByRole("button", { name: "All output" })).toHaveAttribute("aria-pressed", "true");
  expect(right.getByRole("button", { name: "Following" })).toBeVisible();
  expect(right.getByText("ERROR worker failed")).toBeVisible();
});

it("disables clearing while requested and preserves the default clear label", async () => {
  const clear = vi.fn();
  const user = userEvent.setup();
  const { rerender } = render(<LogPanel {...props} onClear={clear} clearing clearLabel="Clear API logs" />);
  const button = screen.getByRole("button", { name: "Clear API logs" });
  expect(button).toBeDisabled();
  await user.click(button);
  expect(clear).not.toHaveBeenCalled();
  rerender(<LogPanel {...props} onClear={clear} />);
  await user.click(screen.getByRole("button", { name: "Clear" }));
  expect(clear).toHaveBeenCalledOnce();
});

it("keeps compact workspace search, stream selection, follow and clear accessible", async () => {
  const user = userEvent.setup();
  const clear = vi.fn();
  render(<LogPanel {...props} compact onClear={clear} clearLabel="Clear output" />);
  expect(screen.queryByRole("heading", { name: "Output" })).not.toBeInTheDocument();
  await user.selectOptions(screen.getByRole("combobox", { name: "Output stream" }), "stderr");
  expect(screen.queryByText("INFO server ready")).not.toBeInTheDocument();
  expect(screen.getByText("ERROR worker failed")).toBeVisible();
  await user.type(screen.getByRole("textbox", { name: "Search logs" }), "missing");
  expect(screen.getByText("No matching output")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Following" }));
  expect(screen.getByRole("button", { name: "Follow" })).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByRole("button", { name: "Clear output" }));
  expect(clear).toHaveBeenCalledOnce();
});

it("uses the shared console for both log layouts", () => {
  const { rerender } = render(<LogPanel {...props} />);
  expect(screen.getByRole("region", { name: "Service logs" })).toHaveClass("ops-console");
  rerender(<LogPanel {...props} compact />);
  expect(screen.getByRole("region", { name: "Service logs" })).toHaveClass("ops-console");
});
