import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import type { DiffFilePatch } from "../lib/types";
import { cn } from "../lib/utils";
import { ChevronRightIcon } from "./icons";
import { Button, Sheet, Spinner } from "./ui";

export function PatchSheet({
  taskId,
  branch,
  baseBranch,
  open,
  onClose,
}: {
  taskId: string;
  branch: string;
  baseBranch: string;
  open: boolean;
  onClose: () => void;
}) {
  const patch = useQuery({
    queryKey: ["patch", taskId],
    queryFn: () => api.patch(taskId),
    enabled: open,
    staleTime: 15_000,
  });

  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!open) {
      setExpanded(new Set());
      return;
    }
    const first = patch.data?.files[0]?.path;
    if (first) setExpanded(new Set([first]));
  }, [open, patch.data]);

  const files = patch.data?.files ?? [];
  const allOpen = files.length > 0 && expanded.size === files.length;

  const toggle = (path: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title="Patch"
      description={`Unified diff ${baseBranch} → ${branch}. Review the actual hunks before merging.`}
      size="wide"
      footer={
        files.length > 0 ? (
          <div className="flex items-center justify-between gap-3">
            <span className="font-mono text-[10px] text-ceniza">
              {files.length} files ·{" "}
              <span className="text-kintsugi">
                +{files.reduce((sum, file) => sum + file.insertions, 0)}
              </span>{" "}
              <span>
                −{files.reduce((sum, file) => sum + file.deletions, 0)}
              </span>
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() =>
                setExpanded(
                  allOpen ? new Set() : new Set(files.map((file) => file.path)),
                )
              }
            >
              {allOpen ? "Collapse all" : "Expand all"}
            </Button>
          </div>
        ) : undefined
      }
    >
      {patch.isLoading ? (
        <div className="flex items-center justify-center gap-2 py-16 font-mono text-xs text-ceniza">
          <Spinner />
          building patch…
        </div>
      ) : patch.isError ? (
        <div className="rounded-lg border border-vermilion/30 bg-vermilion/5 px-3 py-2.5">
          <p className="text-xs leading-relaxed text-vermilion">
            {(patch.error as Error).message}
          </p>
          <p className="mt-1 text-[11px] leading-relaxed text-vermilion/80">
            The worktree may have been removed after merge or discard.
          </p>
        </div>
      ) : files.length === 0 ? (
        <p className="py-10 text-center text-sm text-ceniza">
          No diff between the branch and its base.
        </p>
      ) : (
        <div className="space-y-3">
          {patch.data?.truncated ? (
            <p className="rounded-lg border border-kintsugi/25 bg-kintsugi/5 px-3 py-2 font-mono text-[10px] leading-relaxed text-kintsugi">
              Patch truncated at 512 KB. Review the full diff in the worktree.
            </p>
          ) : null}
          <div className="overflow-hidden rounded-xl border border-hueso/10">
            {files.map((file) => (
              <PatchFile
                key={file.path}
                file={file}
                open={expanded.has(file.path)}
                onToggle={() => toggle(file.path)}
              />
            ))}
          </div>
        </div>
      )}
    </Sheet>
  );
}

function PatchFile({
  file,
  open,
  onToggle,
}: {
  file: DiffFilePatch;
  open: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="border-b border-hueso/10 last:border-b-0">
      <button
        type="button"
        onClick={onToggle}
        className="flex min-h-11 w-full items-center gap-2 bg-raised px-3 py-2.5 text-left transition-colors hover:bg-hueso/5"
        aria-expanded={open}
      >
        <ChevronRightIcon
          className={cn(
            "h-4 w-4 shrink-0 text-ceniza transition-transform",
            open && "rotate-90",
          )}
        />
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-hueso/90">
          {file.path}
        </span>
        <span className="shrink-0 font-mono text-[10px]">
          <span className="text-kintsugi">+{file.insertions}</span>
          <span className="ml-2 text-ceniza">−{file.deletions}</span>
        </span>
      </button>
      {open ? (
        <div className="max-h-[52vh] overflow-auto border-t border-hueso/10 bg-sunken">
          <pre className="min-w-full whitespace-pre-wrap break-words px-3 py-3 font-mono text-[10.5px] leading-[1.65]">
            {file.patch.split("\n").map((line, index) => (
              <span key={index} className={diffLineClass(line)}>
                {line || " "}
                {"\n"}
              </span>
            ))}
          </pre>
        </div>
      ) : null}
    </div>
  );
}

function diffLineClass(line: string): string {
  if (line.startsWith("@@")) {
    return "bg-kintsugi/5 text-kintsugi/80";
  }
  if (
    line.startsWith("+++") ||
    line.startsWith("---") ||
    line.startsWith("index ") ||
    line.startsWith("new file mode") ||
    line.startsWith("deleted file mode") ||
    line.startsWith("similarity index") ||
    line.startsWith("rename ")
  ) {
    return "text-ceniza/60";
  }
  if (line.startsWith("+")) {
    return "bg-kintsugi/5 text-kintsugi";
  }
  if (line.startsWith("-")) {
    return "bg-vermilion/5 text-vermilion/80";
  }
  if (line.startsWith("\\ No newline")) {
    return "text-ceniza/50 italic";
  }
  return "text-[#cfc9bd]";
}
