import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { LogPanel } from "../components/LogPanel";
import type { LogRecord } from "../lib/types";

const record = (
  text: string,
  stream: LogRecord["stream"] = "stderr",
  seq = 1,
): LogRecord => ({
  text,
  stream,
  seq,
  runId: "fixture",
  timestamp: "2026-10-20T00:00:00Z",
  truncated: false,
});
it.each([
  ["2026-10-20T16:35:38+07:00 INF Success status=200", "stderr", "info"],
  ['{"level":"info","message":"error count=0"}', "stderr", "info"],
  ["stderr text without a level", "stderr", "unknown"],
  ["Success: error count=0 status=200", "stderr", "unknown"],
  ["ERROR connection refused", "stdout", "error"],
  ["2026-10-20T16:35:38.123Z ERR request failed", "stderr", "error"],
  ['{"level":"error","message":"request failed"}', "stdout", "error"],
  ["[WARN] retry later", "stderr", "warning"],
  ['{"level":"warning","message":"retry later"}', "stdout", "warning"],
  ['{"message":"error: false"}', "stderr", "unknown"],
  ['{"level":"error"', "stderr", "unknown"],
  ["request payload contains ERROR but is not a level", "stderr", "unknown"],
] as const)(
  "uses severity in %s independently of %s",
  (text, stream, severity) => {
    render(
      <LogPanel
        logs={[record(text, stream)]}
        active={true}
        gap={false}
        onClear={vi.fn()}
      />,
    );
    const row = screen.getByText(text).closest(".log-line")!;
    expect(row).toHaveClass(`severity-${severity}`);
    expect(within(row as HTMLElement).getByText(stream)).toBeVisible();
  },
);
it("keeps informational stderr in the stderr filter with its original text", async () => {
  const user = userEvent.setup();
  render(
    <LogPanel
      logs={[record("INF Success"), record("ERROR failed", "stdout", 2)]}
      active={true}
      gap={false}
      onClear={vi.fn()}
    />,
  );
  await user.click(screen.getByRole("button", { name: "stderr" }));
  expect(screen.getByText("INF Success")).toBeVisible();
  expect(screen.queryByText("ERROR failed")).not.toBeInTheDocument();
});
