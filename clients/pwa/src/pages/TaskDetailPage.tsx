import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ACTIVE_STATES,
  type DiffSummary,
  type Task,
} from "../lib/types";
import { api } from "../lib/api";
import {
  agentLabel,
  cn,
  copyText,
  relativeTime,
  stateLabel,
  truncateMiddle,
} from "../lib/utils";
import { useTaskLog } from "../lib/useTaskLog";
import {
  AlertIcon,
  ArrowLeftIcon,
  CheckIcon,
  CopyIcon,
  GitBranchIcon,
  MergeIcon,
  RetryIcon,
  StopIcon,
  TerminalIcon,
  TrashIcon,
} from "../components/icons";
import {
  Button,
  ConfirmDialog,
  StatePill,
  toast,
} from "../components/ui";
import { PatchSheet } from "../components/PatchSheet";

type ActionKind = "stop" | "retry" | "merge" | "discard";

const actionToasts: Record<ActionKind, string> = {
  stop: "Stop requested",
  retry: "Task queued again",
  merge: "Merged into the base branch",
  discard: "Task discarded",
};

export default function TaskDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [dialog, setDialog] = useState<ActionKind | null>(null);
  const [patchOpen, setPatchOpen] = useState(false);

  const taskQuery = useQuery({
    queryKey: ["task", id],
    queryFn: () => api.task(id!),
    enabled: Boolean(id),
    refetchInterval: (query) => {
      const task = query.state.data;
      return task && ACTIVE_STATES.includes(task.state) ? 3_000 : false;
    },
  });

  const task = taskQuery.data;
  const { log, loaded } = useTaskLog(id);

  const action = useMutation({
    mutationFn: (kind: ActionKind) => {
      if (!id) throw new Error("missing task id");
      switch (kind) {
        case "stop":
          return api.stop(id);
        case "retry":
          return api.retry(id);
        case "merge":
          return api.merge(id);
        case "discard":
          return api.discard(id);
      }
    },
    onSuccess: (updated, kind) => {
      queryClient.setQueryData(["task", updated.id], updated);
      void queryClient.invalidateQueries({ queryKey: ["tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["task", updated.id] });
      toast(
        actionToasts[kind],
        kind === "discard" ? "default" : "success",
      );
      setDialog(null);
    },
    onError: (error: Error) => toast(error.message, "error"),
  });

  const acceptVerification = useMutation({
    mutationFn: () => {
      if (!id) throw new Error("missing task id");
      return api.acceptVerification(id);
    },
    onSuccess: (updated) => {
      queryClient.setQueryData(["task", updated.id], updated);
      void queryClient.invalidateQueries({ queryKey: ["tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["task", updated.id] });
      toast("Verification changes accepted", "success");
    },
    onError: (error: Error) => toast(error.message, "error"),
  });

  const copy = async (value: string, label: string) => {
    const ok = await copyText(value);
    toast(ok ? `${label} copied` : "Copy failed", ok ? "success" : "error");
  };

  if (taskQuery.isLoading) {
    return <TaskDetailSkeleton />;
  }

  if (taskQuery.isError || !task) {
    return (
      <div className="flex flex-col items-center py-20 text-center">
        <img
          src="/brand/oni-hannya.svg"
          alt=""
          className="h-20 w-20 opacity-90"
        />
        <h1 className="mt-5 text-xl font-semibold">Task not found</h1>
        <p className="mt-2 max-w-xs text-sm text-ceniza">
          {taskQuery.isError
            ? (taskQuery.error as Error).message
            : "This task does not exist or was removed."}
        </p>
        <Button className="mt-6" onClick={() => navigate("/")}>
          Back to tasks
        </Button>
      </div>
    );
  }

  const active = ACTIVE_STATES.includes(task.state);
  const review = task.state === "review";
  const failed = task.state === "failed";
  const stopped = task.state === "stopped";
  const verificationBlocked = Boolean(
    task.verification?.tampered && !task.verification.acceptedByHuman,
  );

  return (
    <div className="animate-rise">
      <Link
        to="/"
        className="-ml-2 inline-flex h-10 items-center gap-2 px-2 font-mono text-[10px] uppercase tracking-[0.14em] text-ceniza transition-colors hover:text-hueso"
      >
        <ArrowLeftIcon className="h-4 w-4" />
        Tasks
      </Link>

      <header className="mt-4">
        <div className="flex flex-wrap items-center gap-3">
          <StatePill state={task.state} />
          <span className="font-mono text-[10px] text-ceniza">
            {task.projectName} · {relativeTime(task.createdAt)}
          </span>
        </div>
        <h1 className="mt-4 text-pretty text-[24px] font-semibold leading-tight tracking-[-0.025em] text-hueso md:text-[30px]">
          {task.title}
        </h1>
      </header>

      <Pipeline task={task} />

      {task.verification?.tampered ? (
        <section
          className={cn(
            "mt-5 rounded-xl border p-4",
            verificationBlocked
              ? "border-vermilion/30 bg-vermilion/5"
              : "border-kintsugi/25 bg-kintsugi/5",
          )}
        >
          <div
            className={cn(
              "flex items-center gap-2",
              verificationBlocked ? "text-vermilion" : "text-kintsugi",
            )}
          >
            <AlertIcon className="h-4 w-4" />
            <h2 className="font-mono text-[10px] uppercase tracking-[0.14em]">
              {verificationBlocked
                ? "Verification files changed"
                : "Verification changes accepted"}
            </h2>
          </div>
          <p className="mt-2 text-xs leading-relaxed text-ceniza">
            {verificationBlocked
              ? "The agent modified files that can weaken the check (tests, CI, config, or the verifier). Review the patch and accept explicitly before merging."
              : "A human accepted the changes to the verification files."}
          </p>
          <ul className="mt-3 space-y-1 font-mono text-[10px] text-hueso/80">
            {task.verification.files.map((file) => (
              <li key={file}>{file}</li>
            ))}
          </ul>
          {verificationBlocked && review ? (
            <Button
              variant="secondary"
              size="sm"
              className="mt-4"
              onClick={() => acceptVerification.mutate()}
              loading={acceptVerification.isPending}
            >
              Accept verification changes
            </Button>
          ) : null}
        </section>
      ) : null}

      <section className="mt-6 grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-hueso/10 bg-hueso/10 md:grid-cols-4">
        <Meta label="Branch">
          <button
            type="button"
            className="flex min-w-0 items-center gap-1.5 text-left font-mono text-[11px] text-hueso/90 hover:text-kintsugi"
            onClick={() => void copy(task.branch, "Branch")}
            title={task.branch}
          >
            <GitBranchIcon className="h-3.5 w-3.5 shrink-0 text-ceniza" />
            <span className="truncate">{task.branch}</span>
            <CopyIcon className="h-3 w-3 shrink-0 text-ceniza/60" />
          </button>
        </Meta>
        <Meta label="Base">
          <span className="truncate font-mono text-[11px] text-hueso/90">
            {task.baseBranch}
          </span>
        </Meta>
        <Meta label="Agent">
          <span className="truncate font-mono text-[11px] text-hueso/90">
            {agentLabel(task.agent)}
          </span>
        </Meta>
        <Meta label="Attempt">
          <span className="font-mono text-[11px] text-hueso/90">
            {task.iteration} / {task.maxIterations}
          </span>
        </Meta>
      </section>

      {task.lastError && (failed || stopped) ? (
        <section className="mt-5 rounded-xl border border-vermilion/30 bg-vermilion/5 p-4">
          <div className="flex items-center gap-2 text-vermilion">
            <AlertIcon className="h-4 w-4" />
            <h2 className="font-mono text-[10px] uppercase tracking-[0.14em]">
              Needs attention
            </h2>
          </div>
          <pre className="mt-3 max-h-52 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-vermilion/90">
            {task.lastError}
          </pre>
        </section>
      ) : null}

      {task.diff ? (
        <DiffSection
          diff={task.diff}
          canPatch={!["merged", "discarded"].includes(task.state)}
          onOpenPatch={() => setPatchOpen(true)}
        />
      ) : null}

      {review && !task.diff ? (
        <p className="mt-5 rounded-xl border border-hueso/10 bg-raised px-4 py-3 text-xs text-ceniza">
          No diff was recorded. Inspect the worktree or the log before merging.
        </p>
      ) : null}

      <LogPanel
        log={log}
        loaded={loaded}
        active={active}
        branch={task.branch}
      />

      <details className="group mt-5 rounded-xl border border-hueso/10 bg-raised px-4 py-3">
        <summary className="cursor-pointer list-none font-mono text-[10px] uppercase tracking-[0.14em] text-ceniza transition-colors hover:text-hueso">
          Task brief
          <span className="ml-2 text-ceniza/50 group-open:hidden">+</span>
          <span className="ml-2 hidden text-ceniza/50 group-open:inline">−</span>
        </summary>
        <div className="mt-4 space-y-4">
          <pre className="whitespace-pre-wrap break-words font-sans text-sm leading-relaxed text-hueso/90">
            {task.prompt}
          </pre>
          <div>
            <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
              Verification
            </p>
            <p className="mt-1 break-all font-mono text-[11px] text-hueso/90">
              {task.acceptance}
            </p>
          </div>
          <div>
            <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
              Worktree
            </p>
            <button
              type="button"
              className="mt-1 flex max-w-full items-center gap-2 text-left font-mono text-[11px] text-hueso/90 hover:text-kintsugi"
              onClick={() => void copy(task.worktreePath, "Worktree path")}
              title={task.worktreePath}
            >
              <span className="truncate">
                {truncateMiddle(task.worktreePath, 64)}
              </span>
              <CopyIcon className="h-3 w-3 shrink-0 text-ceniza/60" />
            </button>
          </div>
        </div>
      </details>

      {active ? (
        <ActionBar>
          <div className="hidden min-w-0 flex-1 md:block">
            <p className="truncate font-mono text-[10px] uppercase tracking-[0.14em] text-ceniza">
              {stateLabel(task.state)} · attempt {task.iteration}/
              {task.maxIterations}
            </p>
            <p className="mt-0.5 truncate font-mono text-[10px] text-ceniza/60">
              {task.branch}
            </p>
          </div>
          <Button
            variant="danger"
            className="w-full md:w-auto"
            onClick={() => setDialog("stop")}
            loading={action.isPending && action.variables === "stop"}
          >
            <StopIcon className="h-4 w-4" />
            Stop task
          </Button>
        </ActionBar>
      ) : null}

      {review ? (
        <ActionBar>
          <div className="hidden min-w-0 flex-1 md:block">
            <p className="truncate font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
              Ready for review
            </p>
            <p className="mt-0.5 truncate font-mono text-[10px] text-ceniza/60">
              {task.diff?.files.length ?? 0} files changed · {task.branch}
            </p>
          </div>
          <Button
            variant="danger"
            size="sm"
            className="flex-1 md:flex-none"
            onClick={() => setDialog("discard")}
            loading={action.isPending && action.variables === "discard"}
          >
            <TrashIcon className="h-4 w-4" />
            Discard
          </Button>
          <Button
            variant="secondary"
            size="sm"
            className="flex-1 md:flex-none"
            onClick={() => setDialog("retry")}
            loading={action.isPending && action.variables === "retry"}
          >
            <RetryIcon className="h-4 w-4" />
            Retry
          </Button>
          <Button
            variant="primary"
            size="sm"
            className="flex-[1.4] md:flex-none"
            onClick={() => setDialog("merge")}
            loading={action.isPending && action.variables === "merge"}
            disabled={verificationBlocked}
          >
            <MergeIcon className="h-4 w-4" />
            Merge
          </Button>
        </ActionBar>
      ) : null}

      {failed || stopped ? (
        <ActionBar>
          <div className="hidden min-w-0 flex-1 md:block">
            <p className="truncate font-mono text-[10px] uppercase tracking-[0.14em] text-vermilion">
              {stateLabel(task.state)}
            </p>
            <p className="mt-0.5 truncate font-mono text-[10px] text-ceniza/60">
              The worktree was kept for inspection.
            </p>
          </div>
          <Button
            variant="danger"
            size="sm"
            className="flex-1 md:flex-none"
            onClick={() => setDialog("discard")}
            loading={action.isPending && action.variables === "discard"}
          >
            <TrashIcon className="h-4 w-4" />
            Discard
          </Button>
          <Button
            variant="primary"
            size="sm"
            className="flex-[1.4] md:flex-none"
            onClick={() => setDialog("retry")}
            loading={action.isPending && action.variables === "retry"}
          >
            <RetryIcon className="h-4 w-4" />
            Retry task
          </Button>
        </ActionBar>
      ) : null}

      <PatchSheet
        taskId={task.id}
        branch={task.branch}
        baseBranch={task.baseBranch}
        open={patchOpen}
        onClose={() => setPatchOpen(false)}
      />

      <ConfirmDialog
        open={dialog === "merge"}
        onClose={() => setDialog(null)}
        onConfirm={() => action.mutate("merge")}
        title={`Merge into ${task.baseBranch}?`}
        description="Oni merges with --no-ff. The base branch must be checked out and the repository clean."
        confirmLabel="Merge"
        loading={action.isPending && action.variables === "merge"}
      >
        <div className="rounded-lg border border-hueso/10 bg-sunken p-3">
          <p className="font-mono text-[11px] text-hueso">
            {task.branch} <span className="text-ceniza">→</span>{" "}
            {task.baseBranch}
          </p>
          <p className="mt-2 font-mono text-[10px] text-ceniza">
            {task.diff?.files.length ?? 0} files ·{" "}
            <span className="text-kintsugi">
              +{task.diff?.insertions ?? 0}
            </span>{" "}
            <span>−{task.diff?.deletions ?? 0}</span>
          </p>
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={dialog === "retry"}
        onClose={() => setDialog(null)}
        onConfirm={() => action.mutate("retry")}
        title="Run the agent again?"
        description="Oni resumes in the same worktree and gives the agent one more attempt. The previous output is kept in the log."
        confirmLabel="Retry"
        loading={action.isPending && action.variables === "retry"}
      />

      <ConfirmDialog
        open={dialog === "stop"}
        onClose={() => setDialog(null)}
        onConfirm={() => action.mutate("stop")}
        title="Stop this task?"
        description="The agent process tree is killed. The worktree stays on disk so you can inspect or discard it."
        confirmLabel="Stop"
        tone="danger"
        loading={action.isPending && action.variables === "stop"}
      />

      <ConfirmDialog
        open={dialog === "discard"}
        onClose={() => setDialog(null)}
        onConfirm={() => action.mutate("discard")}
        title="Discard this task?"
        description="The worktree and its branch are deleted. This cannot be undone."
        confirmLabel="Discard"
        tone="danger"
        loading={action.isPending && action.variables === "discard"}
      >
        <p className="rounded-lg border border-vermilion/20 bg-vermilion/5 px-3 py-2 font-mono text-[10px] text-vermilion/90">
          {task.branch}
        </p>
      </ConfirmDialog>
    </div>
  );
}

function Meta({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0 bg-sumi px-3 py-3">
      <p className="font-mono text-[9px] uppercase tracking-[0.16em] text-ceniza">
        {label}
      </p>
      <div className="mt-1.5 min-w-0">{children}</div>
    </div>
  );
}

function Pipeline({ task }: { task: Task }) {
  const steps = ["Act", "Verify", "Review"];
  let activeIndex = -1;

  if (task.state === "queued" || task.state === "acting" || task.state === "retrying") {
    activeIndex = 0;
  } else if (task.state === "verifying") {
    activeIndex = 1;
  } else if (task.state === "review" || task.state === "merged") {
    activeIndex = 2;
  }

  const failedIndex =
    task.state === "failed"
      ? task.lastError?.toLowerCase().includes("verification")
        ? 1
        : 0
      : -1;

  const allDone = task.state === "merged";

  return (
    <section className="mt-6 flex items-center gap-2 md:gap-3">
      {steps.map((step, index) => {
        const done = allDone || (activeIndex >= 0 && index < activeIndex);
        const current = !allDone && index === activeIndex;
        const isFailed = index === failedIndex;

        return (
          <div key={step} className="flex min-w-0 flex-1 items-center gap-2">
            <span
              className={cn(
                "flex h-6 w-6 shrink-0 items-center justify-center rounded-full border font-mono text-[9px]",
                isFailed
                  ? "border-vermilion/60 bg-vermilion/10 text-vermilion"
                  : done
                    ? "border-kintsugi/50 bg-kintsugi/10 text-kintsugi"
                    : current
                      ? "border-kintsugi text-kintsugi"
                      : "border-hueso/15 text-ceniza/50",
              )}
            >
              {done ? <CheckIcon className="h-3.5 w-3.5" /> : index + 1}
            </span>
            <span
              className={cn(
                "truncate font-mono text-[10px] uppercase tracking-[0.12em]",
                isFailed
                  ? "text-vermilion"
                  : done || current
                    ? "text-hueso"
                    : "text-ceniza/50",
              )}
            >
              {step}
            </span>
            {index < steps.length - 1 ? (
              <span
                className={cn(
                  "h-px flex-1",
                  done ? "bg-kintsugi/30" : "bg-hueso/10",
                )}
              />
            ) : null}
          </div>
        );
      })}
    </section>
  );
}

function DiffSection({
  diff,
  canPatch,
  onOpenPatch,
}: {
  diff: DiffSummary;
  canPatch: boolean;
  onOpenPatch: () => void;
}) {
  return (
    <section className="mt-5 overflow-hidden rounded-xl border border-hueso/10 bg-raised">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-hueso/10 px-4 py-3">
        <div className="flex items-center gap-3">
          <h2 className="font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
            Changes
          </h2>
          <p className="font-mono text-[10px] text-ceniza">
            {diff.files.length} files ·{" "}
            <span className="text-kintsugi">+{diff.insertions}</span>{" "}
            <span>−{diff.deletions}</span>
          </p>
        </div>
        {canPatch && diff.files.length > 0 ? (
          <Button variant="secondary" size="sm" onClick={onOpenPatch}>
            View patch
          </Button>
        ) : null}
      </header>
      {diff.files.length === 0 ? (
        <p className="px-4 py-4 text-xs text-ceniza">
          No changes were detected between the worktree and the base branch.
        </p>
      ) : (
        <ul className="max-h-72 divide-y divide-hueso/8 overflow-auto">
          {diff.files.map((file) => (
            <li
              key={file.path}
              className="flex items-center gap-3 px-4 py-2.5"
            >
              <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-hueso/85">
                {file.path}
              </span>
              <span className="shrink-0 font-mono text-[10px]">
                <span className="text-kintsugi">+{file.insertions}</span>
                <span className="ml-2 text-ceniza">−{file.deletions}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function LogPanel({
  log,
  loaded,
  active,
  branch,
}: {
  log: string;
  loaded: boolean;
  active: boolean;
  branch: string;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(true);

  useEffect(() => {
    const element = scrollRef.current;
    if (element && follow) {
      element.scrollTop = element.scrollHeight;
    }
  }, [log, follow]);

  const onScroll = () => {
    const element = scrollRef.current;
    if (!element) return;
    const distance =
      element.scrollHeight - element.scrollTop - element.clientHeight;
    setFollow(distance < 48);
  };

  return (
    <section className="mt-5 overflow-hidden rounded-xl border border-hueso/10 bg-sunken">
      <header className="flex items-center justify-between gap-3 border-b border-hueso/10 px-3 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <TerminalIcon className="h-3.5 w-3.5 text-kintsugi" />
          <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-ceniza">
            Output
          </span>
          <span className="hidden truncate font-mono text-[10px] text-ceniza/50 sm:inline">
            {branch}
          </span>
        </div>
        <div className="flex items-center gap-3">
          {active ? (
            <span className="flex items-center gap-1.5 font-mono text-[9px] uppercase tracking-[0.14em] text-kintsugi">
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-kintsugi motion-reduce:animate-none" />
              live
            </span>
          ) : null}
          {!follow ? (
            <button
              type="button"
              className="font-mono text-[9px] uppercase tracking-[0.14em] text-kintsugi underline underline-offset-4"
              onClick={() => {
                setFollow(true);
                const element = scrollRef.current;
                if (element) element.scrollTop = element.scrollHeight;
              }}
            >
              Follow
            </button>
          ) : null}
        </div>
      </header>
      <div
        ref={scrollRef}
        onScroll={onScroll}
        className="max-h-[52vh] min-h-36 overflow-auto px-3 py-3"
      >
        {log ? (
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-[1.7] text-[#cfc9bd]">
            {log.split("\n").map((line, index) => (
              <span
                key={index}
                className={
                  line.startsWith("== ") ? "text-kintsugi/80" : undefined
                }
              >
                {line}
                {"\n"}
              </span>
            ))}
          </pre>
        ) : (
          <div className="flex h-28 items-center justify-center gap-2 font-mono text-[10px] uppercase tracking-[0.14em] text-ceniza/60">
            {loaded ? "No output yet" : "Reading log"}
            <span className="inline-block h-3 w-1.5 animate-pulse bg-kintsugi/70 motion-reduce:animate-none" />
          </div>
        )}
      </div>
    </section>
  );
}

function ActionBar({ children }: { children: React.ReactNode }) {
  return (
    <div className="fixed inset-x-0 bottom-0 z-40 border-t border-hueso/10 bg-sumi/95 md:left-60">
      <div className="mx-auto flex w-full max-w-4xl items-center gap-2 px-4 pb-[max(env(safe-area-inset-bottom),0.75rem)] pt-3">
        {children}
      </div>
    </div>
  );
}

function TaskDetailSkeleton() {
  return (
    <div className="animate-pulse space-y-5 motion-reduce:animate-none">
      <div className="h-8 w-24 rounded bg-hueso/5" />
      <div className="h-5 w-20 rounded bg-hueso/5" />
      <div className="h-9 w-2/3 rounded bg-hueso/5" />
      <div className="h-12 rounded bg-hueso/5" />
      <div className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-hueso/10 bg-hueso/10 md:grid-cols-4">
        {[0, 1, 2, 3].map((index) => (
          <div key={index} className="h-16 bg-sumi" />
        ))}
      </div>
      <div className="h-56 rounded-xl border border-hueso/10 bg-sunken" />
    </div>
  );
}
