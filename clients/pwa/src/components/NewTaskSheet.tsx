import { useEffect, useRef, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router-dom";
import { api } from "../lib/api";
import type { AgentPreset, NewTaskInput } from "../lib/types";
import { agentLabel } from "../lib/utils";
import { PlusIcon } from "./icons";
import {
  Button,
  Field,
  Sheet,
  inputClass,
  selectClass,
  textareaClass,
  toast,
} from "./ui";

export function NewTaskSheet({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const projects = useQuery({
    queryKey: ["projects"],
    queryFn: api.projects,
    enabled: open,
  });

  const [projectId, setProjectId] = useState("");
  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [acceptance, setAcceptance] = useState("");
  const [agent, setAgent] = useState<AgentPreset>("opencode");
  const [customCommand, setCustomCommand] = useState("");
  const [maxIterations, setMaxIterations] = useState(3);
  const [error, setError] = useState<string | null>(null);
  const initialized = useRef(false);

  const selectedProject = projects.data?.find(
    (project) => project.id === projectId,
  );

  const canSubmit =
    Boolean(selectedProject) &&
    Boolean(title.trim()) &&
    Boolean(prompt.trim()) &&
    Boolean(acceptance.trim()) &&
    (agent !== "custom" || Boolean(customCommand.trim()));

  useEffect(() => {
    if (!open) {
      initialized.current = false;
      return;
    }
    if (initialized.current || !projects.isFetched) return;
    initialized.current = true;

    setTitle("");
    setPrompt("");
    setError(null);
    setMaxIterations(3);

    const first = projects.data?.[0];
    if (first) {
      setProjectId(first.id);
      setAcceptance(first.verifyCommand);
      setAgent(first.agent);
      setCustomCommand(first.customCommand ?? "");
    } else {
      setProjectId("");
      setAcceptance("");
    }
  }, [open, projects.data, projects.isFetched]);

  const createTask = useMutation({
    mutationFn: (input: NewTaskInput) => api.createTask(input),
    onSuccess: (task) => {
      void queryClient.invalidateQueries({ queryKey: ["tasks"] });
      toast("Task dispatched", "success");
      onClose();
      navigate(`/tasks/${task.id}`);
    },
    onError: (mutationError: Error) => setError(mutationError.message),
  });

  const chooseProject = (value: string) => {
    setProjectId(value);
    const project = projects.data?.find((item) => item.id === value);
    if (project) {
      setAcceptance(project.verifyCommand);
      setAgent(project.agent);
      setCustomCommand(project.customCommand ?? "");
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    if (!projectId) {
      setError("Connect a repository first.");
      return;
    }
    if (!title.trim() || !prompt.trim()) {
      setError("Title and prompt are required.");
      return;
    }
    createTask.mutate({
      projectId,
      title: title.trim(),
      prompt: prompt.trim(),
      acceptance: acceptance.trim(),
      agent,
      customCommand: agent === "custom" ? customCommand.trim() : null,
      maxIterations,
    });
  };

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title="Dispatch a task"
      description="Oni creates an isolated worktree, runs the agent, then verifies the result before asking you to review."
      footer={
        projects.data && projects.data.length > 0 ? (
          <div>
            {error ? (
              <p className="mb-3 rounded-lg border border-vermilion/30 bg-vermilion/5 px-3 py-2 text-xs leading-relaxed text-vermilion">
                {error}
              </p>
            ) : null}
            <Button
              type="submit"
              form="new-task-form"
              variant="primary"
              className="w-full"
              loading={createTask.isPending}
              disabled={!canSubmit}
            >
              <PlusIcon className="h-4 w-4" />
              Dispatch task
            </Button>
            <p className="mt-3 text-center font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza/60">
              isolated worktree · no direct writes to{" "}
              {selectedProject?.baseBranch ?? "main"}
            </p>
          </div>
        ) : undefined
      }
    >
      {projects.isLoading ? (
        <p className="py-8 text-center font-mono text-xs text-ceniza">
          loading projects…
        </p>
      ) : projects.data?.length === 0 ? (
        <div className="space-y-5 py-4">
          <div className="rounded-lg border border-kintsugi/20 bg-kintsugi/5 p-4">
            <p className="text-sm font-medium text-hueso">
              No repository connected
            </p>
            <p className="mt-1 text-pretty text-xs leading-relaxed text-ceniza">
              Add a local git repository first. Oni needs a base branch, an
              agent and a command that proves the work is correct.
            </p>
          </div>
          <Link
            to="/projects"
            onClick={onClose}
            className="inline-flex h-11 w-full items-center justify-center rounded-lg border border-kintsugi bg-kintsugi text-sm font-medium text-sumi"
          >
            Connect a repository
          </Link>
        </div>
      ) : (
        <form id="new-task-form" onSubmit={submit} className="space-y-5">
          <Field label="Repository" htmlFor="task-project">
            <div className="relative">
              <select
                id="task-project"
                className={selectClass}
                value={projectId}
                onChange={(event) => chooseProject(event.target.value)}
              >
                {projects.data?.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name} · {project.baseBranch}
                  </option>
                ))}
              </select>
              <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center font-mono text-[10px] text-ceniza">
                ▾
              </span>
            </div>
          </Field>

          <Field label="Title" htmlFor="task-title">
            <input
              id="task-title"
              className={inputClass}
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="Fix the flaky auth test"
              maxLength={160}
              autoFocus
            />
          </Field>

          <Field
            label="Prompt"
            htmlFor="task-prompt"
            hint="What should change, and what should not. The agent works only inside the worktree."
          >
            <textarea
              id="task-prompt"
              className={textareaClass}
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder="Reproduce the failure, fix the root cause, and keep the public API unchanged."
            />
          </Field>

          <div className="grid gap-5 sm:grid-cols-2">
            <Field label="Agent" htmlFor="task-agent">
              <div className="relative">
                <select
                  id="task-agent"
                  className={selectClass}
                  value={agent}
                  onChange={(event) =>
                    setAgent(event.target.value as AgentPreset)
                  }
                >
                  {(["opencode", "claude", "custom"] as AgentPreset[]).map(
                    (preset) => (
                      <option key={preset} value={preset}>
                        {agentLabel(preset)}
                      </option>
                    ),
                  )}
                </select>
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center font-mono text-[10px] text-ceniza">
                  ▾
                </span>
              </div>
            </Field>

            <Field label="Max attempts" htmlFor="task-attempts">
              <div className="relative">
                <select
                  id="task-attempts"
                  className={selectClass}
                  value={maxIterations}
                  onChange={(event) =>
                    setMaxIterations(Number(event.target.value))
                  }
                >
                  {[1, 2, 3, 4, 5, 6].map((value) => (
                    <option key={value} value={value}>
                      {value} {value === 1 ? "attempt" : "attempts"}
                    </option>
                  ))}
                </select>
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center font-mono text-[10px] text-ceniza">
                  ▾
                </span>
              </div>
            </Field>
          </div>

          {agent === "custom" ? (
            <Field
              label="Custom command"
              htmlFor="task-custom"
              hint="Executed with the worktree as cwd. The prompt is sent to stdin."
            >
              <input
                id="task-custom"
                className={inputClass}
                value={customCommand}
                onChange={(event) => setCustomCommand(event.target.value)}
                placeholder="my-agent --headless"
              />
            </Field>
          ) : null}

          <Field
            label="Verification command"
            htmlFor="task-acceptance"
            hint="Runs inside the worktree after the agent exits. A non-zero exit triggers a retry."
          >
            <input
              id="task-acceptance"
              className={`${inputClass} font-mono text-[13px]`}
              value={acceptance}
              onChange={(event) => setAcceptance(event.target.value)}
              placeholder="npm test"
            />
          </Field>
        </form>
      )}
    </Sheet>
  );
}
