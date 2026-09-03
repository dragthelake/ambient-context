import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DayActions } from "./DayActions";
import { DayHeader } from "./DayHeader";
import { KnowledgePane } from "./KnowledgePane";
import { NotesPane } from "./NotesPane";
import { RawPane } from "./RawPane";
import { WebsitesPane } from "./WebsitesPane";
import type { DayEntry, DayFile, KnowledgeSection } from "../lib/days";

export type { DayEntry };

export type DayStats = { blocks: number; hours: number };

export type Outcome = {
  when: string;
  date: string;
  ok: boolean;
  message: string;
  /// How long the whole run for that day took. Absent on an outcome
  /// recorded by a build before the runner measured it.
  took_ms: number | null;
};

/// Context is the day as it was captured, Knowledge the wiki built from
/// it, Notes the written day.
export type DayMode = "context" | "knowledge" | "notes";

/// Whether a keystroke belongs to whatever the user is typing into rather
/// than to the day-navigation shortcuts. The propose popover's textarea and
/// the day picker's inputs both sit inside this view, and a global keydown
/// listener sees their keys too.
export function isTypingTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

/// Compare two outcomes by value: `job_status` returns a fresh object every
/// call, and setting an equal-but-new one re-renders the whole day for nothing.
function sameOutcome(a: Outcome | null, b: Outcome | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.when === b.when &&
    a.date === b.date &&
    a.ok === b.ok &&
    a.message === b.message &&
    a.took_ms === b.took_ms
  );
}

export type JobStatus = "queued" | "running" | "done" | "failed";

export type JobState = {
  id: string;
  date: string;
  status: JobStatus;
  stderr: string | null;
  step: string | null;
};

export type SummaryState =
  | { kind: "none" }
  | { kind: "queued" }
  | { kind: "running" }
  | { kind: "generated"; at: string; tookMs: number | null }
  | { kind: "failed"; message: string };

/// The runner prefixes failures with "{date} failed: ". In the day header
/// that date is already in the title, so strip the prefix when present.
function failureReason(message: string, date: string): string {
  const prefix = `${date} failed: `;
  return message.startsWith(prefix) ? message.slice(prefix.length) : message;
}

const BLOCK_HEADING = /^## (\d{2}):(\d{2})[-–](\d{2}):(\d{2})/;

export function dayStats(dayMarkdown: string | null): DayStats {
  if (!dayMarkdown) return { blocks: 0, hours: 0 };
  let blocks = 0;
  let minutes = 0;
  for (const line of dayMarkdown.split("\n")) {
    const match = BLOCK_HEADING.exec(line);
    if (!match) continue;
    blocks += 1;
    const start = Number(match[1]) * 60 + Number(match[2]);
    const end = Number(match[3]) * 60 + Number(match[4]);
    // A block that crosses midnight is written to the day it started on.
    minutes += end >= start ? end - start : 24 * 60 - start + end;
  }
  return { blocks, hours: minutes / 60 };
}

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

function shift(date: string, days: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const next = new Date(y, m - 1, d + days);
  const month = String(next.getMonth() + 1).padStart(2, "0");
  const day = String(next.getDate()).padStart(2, "0");
  return `${next.getFullYear()}-${month}-${day}`;
}

export function DayView({ date }: { date?: string } = {}) {
  const [selected, setSelected] = useState(todayIso);
  const [days, setDays] = useState<DayEntry[]>([]);
  const [dayMarkdown, setDayMarkdown] = useState<string | null>(null);
  const [summaryMarkdown, setSummaryMarkdown] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [hasAgent, setHasAgent] = useState(false);
  const [job, setJob] = useState<JobState | null>(null);
  // A run you started yourself, and the message it failed with. The
  // scheduler's last outcome is a different fact about a possibly different
  // day, and must never stand in for this one.
  const [manualFailure, setManualFailure] = useState<{
    date: string;
    message: string;
    when: string;
  } | null>(null);
  // Context is the default for today: today is what you are still recording.
  const [mode, setMode] = useState<DayMode>(() =>
    selected === todayIso() ? "context" : "notes",
  );
  const [rawFile, setRawFile] = useState<DayFile>("apps");
  const [section, setSection] = useState<KnowledgeSection>("people.md");
  const [knowledgeRefresh, setKnowledgeRefresh] = useState(0);

  // A plain setter since the calendar rail went: nothing else has to be
  // kept in step with the selected day. Wrapped anyway so the call sites
  // and their dependency arrays did not all have to change.
  const selectDate = useCallback((date: string) => setSelected(date), []);

  // The window can be opened for a particular day, by the tray or by an
  // agent over MCP. Taken once on mount, and listened for while open.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const pending = await invoke<string | null>("take_pending_day");
        if (!cancelled && pending) selectDate(pending);
      } catch {
        // An older build without the command: today is the right answer.
      }
    })();
    const unlisten = listen<string>("open-day", (event) => {
      if (event.payload) selectDate(event.payload);
    });
    return () => {
      cancelled = true;
      void unlisten.then((off) => off()).catch(() => undefined);
    };
  }, [selectDate]);

  // The same effect the open-day event has, on an internal route: the
  // Overview map opens a day without going through Tauri.
  useEffect(() => {
    if (date) selectDate(date);
  }, [date, selectDate]);

  // Every recorded day, not one month of them. The header only wants the
  // entry for the selected day, and this is the list the Overview map
  // already reads.
  const refreshDays = useCallback(async () => {
    setDays(await invoke<DayEntry[]>("list_days"));
  }, []);

  useEffect(() => {
    void refreshDays();
  }, [refreshDays]);

  // The last completed run, read on its own schedule. It is deliberately not
  // a dependency of the day load: that is what made the two re-enter each
  // other once any run had recorded an outcome.
  const refreshOutcome = useCallback(async () => {
    const status = await invoke<Outcome | null>("job_status");
    setOutcome((current) => (sameOutcome(current, status) ? current : status));
    // A later run for the same day supersedes the manual failure; a run for
    // any other day leaves it alone.
    setManualFailure((current) =>
      current && status && status.date === current.date && status.when > current.when
        ? null
        : current,
    );
  }, []);

  useEffect(() => {
    void refreshOutcome();
  }, [refreshOutcome]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [day, summary, settings] = await Promise.all([
        invoke<string | null>("read_day", { date: selected, file: "apps" }),
        invoke<string | null>("read_summary", { date: selected }),
        invoke<{ agent: unknown }>("get_settings"),
      ]);
      if (cancelled) return;
      setDayMarkdown(day);
      setSummaryMarkdown(summary);
      setHasAgent(settings.agent !== null);
      // Context is the default for today; Notes for a past day that has them.
      setMode((current) =>
        selected === todayIso() ? current : summary ? "notes" : "context",
      );
    })();
    return () => {
      cancelled = true;
    };
  }, [selected]);

  // Today's file grows while you look at it; refresh it live.
  useEffect(() => {
    if (selected !== todayIso()) return;
    const id = setInterval(async () => {
      const day = await invoke<string | null>("read_day", { date: selected, file: "apps" });
      setDayMarkdown(day);
      void refreshOutcome();
    }, 5000);
    return () => clearInterval(id);
  }, [selected, refreshOutcome]);

  const onPrev = useCallback(
    () => selectDate(shift(selected, -1)),
    [selectDate, selected],
  );
  const onNext = useCallback(
    () => selectDate(shift(selected, 1)),
    [selectDate, selected],
  );
  const onToday = useCallback(() => selectDate(todayIso()), [selectDate]);

  // A window listener sees keystrokes a focused field is already using, so
  // typing a note in the propose popover's textarea would otherwise jump the
  // day. Every kind of editable target is skipped, not just <input>.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTypingTarget(event.target)) return;
      if (event.key === "ArrowLeft") onPrev();
      if (event.key === "ArrowRight") onNext();
      if (event.key.toLowerCase() === "t") onToday();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onPrev, onNext, onToday]);

  const reloadDay = useCallback(async () => {
    const [day, summary] = await Promise.all([
      invoke<string | null>("read_day", { date: selected, file: "apps" }),
      invoke<string | null>("read_summary", { date: selected }),
    ]);
    setDayMarkdown(day);
    setSummaryMarkdown(summary);
    await refreshOutcome();
    void refreshDays();
  }, [selected, refreshDays, refreshOutcome]);

  // Runs are queued and serial. Either command returns a job id straight
  // away; the view follows that one job and nobody else's. `summarise_now`
  // is the whole pipeline (knowledge, then notes); `ingest_now` stops after
  // the knowledge. `force` is what separates Regenerate from Generate: a
  // day whose calls were already accepted is otherwise left alone.
  const startRun = useCallback(
    async (command: "summarise_now" | "ingest_now", force: boolean) => {
      try {
        const started = await invoke<{ job_id: string }>(command, {
          date: selected,
          force,
        });
        setJob({
          id: started.job_id,
          date: selected,
          status: "queued",
          stderr: null,
          step: null,
        });
        setManualFailure((current) =>
          current && current.date === selected ? null : current,
        );
      } catch (error) {
        setJob({
          id: "",
          date: selected,
          status: "failed",
          stderr: String(error),
          step: null,
        });
        setManualFailure({
          date: selected,
          message: String(error),
          when: new Date().toISOString(),
        });
      }
    },
    [selected],
  );

  const onProcess = useCallback(
    (force: boolean) => {
      setMode("notes");
      void startRun("summarise_now", force);
    },
    [startRun],
  );

  const onGenerateKnowledge = useCallback(
    (force: boolean) => {
      setMode("knowledge");
      void startRun("ingest_now", force);
    },
    [startRun],
  );

  const jobId = job && job.date === selected ? job.id : null;
  const jobStatus = job && job.date === selected ? job.status : null;
  const pending = jobStatus === "queued" || jobStatus === "running";

  useEffect(() => {
    if (!jobId || !pending) return;
    let cancelled = false;
    const id = setInterval(() => {
      void (async () => {
        const state = await invoke<JobState | null>("job_state", { jobId });
        if (cancelled || !state) return;
        setJob((current) =>
          current &&
          current.id === state.id &&
          current.status === state.status &&
          current.step === (state.step ?? null)
            ? current
            : {
                id: state.id,
                date: state.date,
                status: state.status,
                stderr: state.stderr,
                step: state.step ?? null,
              },
        );
        if (state.status === "failed") {
          setManualFailure({
            date: state.date,
            message: state.stderr ?? "The run failed.",
            when: new Date().toISOString(),
          });
        }
        if (state.status === "done") {
          setManualFailure((current) =>
            current && current.date === state.date ? null : current,
          );
          setKnowledgeRefresh((n) => n + 1);
          await reloadDay();
        }
      })();
    }, 2000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [jobId, pending, reloadDay]);

  const entry = useMemo(
    () => days.find((d) => d.date === selected) ?? null,
    [days, selected],
  );
  const stats = useMemo(() => dayStats(dayMarkdown), [dayMarkdown]);

  const running = pending;

  const summary: SummaryState = useMemo(() => {
    if (jobStatus === "queued") return { kind: "queued" };
    if (jobStatus === "running") return { kind: "running" };
    if (manualFailure && manualFailure.date === selected) {
      return { kind: "failed", message: manualFailure.message };
    }
    if (summaryMarkdown) {
      const mine = outcome && outcome.date === selected ? outcome : null;
      return { kind: "generated", at: mine?.when ?? "", tookMs: mine?.took_ms ?? null };
    }
    if (outcome && outcome.date === selected && !outcome.ok) {
      return {
        kind: "failed",
        message: failureReason(outcome.message, selected),
      };
    }
    return { kind: "none" };
  }, [summaryMarkdown, outcome, selected, jobStatus, manualFailure]);

  const hasNotes = summaryMarkdown !== null;
  const hasKnowledge = entry?.has_kb ?? false;

  return (
    <div className="day-view">
      <div className="day-main">
        <DayHeader
          date={selected}
          entry={entry}
          stats={stats}
          summary={summary}
          mode={mode}
          onMode={setMode}
          rawFile={rawFile}
          onRawFile={setRawFile}
          section={section}
          onSection={setSection}
          onPrev={onPrev}
          onNext={onNext}
          onToday={onToday}
          step={job && job.date === selected ? job.step : null}
        />
        {mode === "notes" ? (
          <NotesPane
            markdown={summaryMarkdown}
            hasCapture={entry?.has_capture ?? false}
            hasAgent={hasAgent}
            running={running}
            step={job && job.date === selected ? job.step : null}
            onGenerate={() => onProcess(hasNotes)}
            date={selected}
          />
        ) : mode === "knowledge" ? (
          <KnowledgePane
            date={selected}
            section={section}
            refreshKey={knowledgeRefresh}
            running={running}
            step={job && job.date === selected ? job.step : null}
            hasAgent={hasAgent}
            onGenerate={() => onGenerateKnowledge(hasKnowledge)}
          />
        ) : rawFile === "websites" ? (
          <WebsitesPane date={selected} />
        ) : (
          // The pane is only ever mounted in Context mode, and its own
          // scroll restore keys off that.
          <RawPane date={selected} mode="raw" file={rawFile} />
        )}
        <DayActions
          date={selected}
          mode={mode}
          rawFile={rawFile}
          section={section}
          running={running}
          hasAgent={hasAgent}
          hasKnowledge={hasKnowledge}
          hasNotes={hasNotes}
          onProcess={onProcess}
          onGenerateKnowledge={onGenerateKnowledge}
        />
      </div>
    </div>
  );
}
