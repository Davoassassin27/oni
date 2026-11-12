import { Link } from "react-router-dom";
import type { Task } from "../lib/types";
import { agentLabel, cn, relativeTime, stateTone } from "../lib/utils";
import { ChevronRightIcon } from "./icons";
import { StatePill } from "./ui";

const barTones = {
  active: "bg-kintsugi",
  review: "bg-hueso/50",
  done: "bg-ceniza/30",
  danger: "bg-vermilion",
} as const;

export function TaskCard({ task }: { task: Task }) {
  const tone = stateTone(task.state);

  return (
    <Link
      to={`/tasks/${task.id}`}
      className="group relative block overflow-hidden rounded-xl border border-hueso/10 bg-raised p-4 pl-5 transition-colors hover:border-hueso/20 hover:bg-[#171719]"
    >
      <span
        className={cn(
          "absolute inset-y-4 left-0 w-0.5 rounded-r-full",
          barTones[tone],
        )}
      />
      <div className="flex items-center justify-between gap-3">
        <StatePill state={task.state} />
        <span className="shrink-0 font-mono text-[10px] text-ceniza/70">
          {relativeTime(task.createdAt)}
        </span>
      </div>

      <h2 className="mt-3 text-pretty text-[15px] font-medium leading-snug tracking-[-0.01em] text-hueso">
        {task.title}
      </h2>

      <div className="mt-3 flex items-center gap-2 font-mono text-[10px] text-ceniza">
        <span className="truncate">{task.projectName}</span>
        <span className="text-ceniza/40">·</span>
        <span className="shrink-0">{agentLabel(task.agent)}</span>
        {task.iteration > 0 ? (
          <>
            <span className="text-ceniza/40">·</span>
            <span className="shrink-0">
              {task.iteration}/{task.maxIterations}
            </span>
          </>
        ) : null}
        <ChevronRightIcon className="ml-auto h-4 w-4 shrink-0 text-ceniza/50 transition-transform group-hover:translate-x-0.5 group-hover:text-hueso" />
      </div>
    </Link>
  );
}
