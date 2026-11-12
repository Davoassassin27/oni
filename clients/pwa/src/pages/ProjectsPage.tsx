import { useEffect, useRef, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import type { AgentPreset, NewProjectInput, Project } from "../lib/types";
import { agentLabel } from "../lib/utils";
import { FolderIcon, PencilIcon, PlusIcon, TrashIcon } from "../components/icons";
import {
  Button,
  ConfirmDialog,
  Field,
  Sheet,
  inputClass,
  selectClass,
  textareaClass,
  toast,
} from "../components/ui";

export default function ProjectsPage() {
  const queryClient = useQueryClient();
  const [sheetOpen, setSheetOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [deletingProject, setDeletingProject] = useState<Project | null>(null);

  const projects = useQuery({
    queryKey: ["projects"],
    queryFn: api.projects,
  });

  const deleteProject = useMutation({
    mutationFn: (project: Project) => api.deleteProject(project.id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      toast("Project removed", "success");
      setDeletingProject(null);
    },
    onError: (error: Error) => toast(error.message, "error"),
  });

  const openCreate = () => {
    setEditingProject(null);
    setSheetOpen(true);
  };

  const openEdit = (project: Project) => {
    setEditingProject(project);
    setSheetOpen(true);
  };

  const closeSheet = () => {
    setSheetOpen(false);
    setEditingProject(null);
  };

  return (
    <div className="animate-rise">
      <header className="mb-8">
        <p className="descriptor">Repositories</p>
        <div className="mt-3 flex items-end justify-between gap-4">
          <div>
            <h1 className="text-[28px] font-semibold leading-none tracking-[-0.03em] text-hueso md:text-[32px]">
              Projects
            </h1>
            <p className="mt-2 text-sm text-ceniza">
              Each task gets a fresh worktree from these branches.
            </p>
          </div>
          <div className="hidden shrink-0 md:block">
            <Button
              variant="primary"
              onClick={openCreate}
            >
              <PlusIcon className="h-4 w-4" />
              Add project
            </Button>
          </div>
        </div>
      </header>

      {projects.isLoading ? (
        <div className="space-y-3">
          {[0, 1].map((index) => (
            <div
              key={index}
              className="h-36 animate-pulse rounded-xl border border-hueso/8 bg-raised/60 motion-reduce:animate-none"
            />
          ))}
        </div>
      ) : projects.data?.length === 0 ? (
        <div className="flex flex-col items-center rounded-2xl border border-dashed border-hueso/15 px-6 py-14 text-center">
          <img
            src="/brand/oni-hannya.svg"
            alt=""
            className="h-24 w-24 opacity-95"
          />
          <h2 className="mt-6 text-lg font-semibold tracking-tight text-hueso">
            No repositories yet
          </h2>
          <p className="mt-2 max-w-sm text-pretty text-sm leading-relaxed text-ceniza">
            Point Oni at a local git repository. It will create branches and
            worktrees, run your agent, and verify the result before review.
          </p>
          <Button
            variant="primary"
            className="mt-6"
            onClick={openCreate}
          >
            <PlusIcon className="h-4 w-4" />
            Connect a repository
          </Button>
        </div>
      ) : (
        <div className="space-y-3">
          {projects.data?.map((project, index) => (
            <article
              key={project.id}
              className="animate-rise rounded-xl border border-hueso/10 bg-raised p-4"
              style={{ animationDelay: `${Math.min(index, 6) * 35}ms` }}
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <FolderIcon className="h-4 w-4 shrink-0 text-kintsugi" />
                    <h2 className="truncate text-[15px] font-medium text-hueso">
                      {project.name}
                    </h2>
                  </div>
                  <p
                    className="mt-1.5 truncate font-mono text-[10px] text-ceniza"
                    title={project.path}
                  >
                    {project.path}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  <span className="rounded-md border border-hueso/15 px-2 py-1 font-mono text-[9px] uppercase tracking-[0.12em] text-ceniza">
                    {agentLabel(project.agent)}
                  </span>
                  <button
                    type="button"
                    aria-label={`Edit ${project.name}`}
                    title="Edit project"
                    onClick={() => openEdit(project)}
                    className="flex h-11 w-11 items-center justify-center rounded-lg text-ceniza transition-colors hover:bg-hueso/5 hover:text-hueso"
                  >
                    <PencilIcon className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    aria-label={`Delete ${project.name}`}
                    title="Delete project"
                    onClick={() => setDeletingProject(project)}
                    className="flex h-11 w-11 items-center justify-center rounded-lg text-ceniza transition-colors hover:bg-vermilion/10 hover:text-vermilion"
                  >
                    <TrashIcon className="h-4 w-4" />
                  </button>
                </div>
              </div>

              <div className="mt-4 grid gap-3 border-t border-hueso/10 pt-3 sm:grid-cols-2">
                <div className="min-w-0">
                  <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
                    Base branch
                  </p>
                  <p className="mt-1 truncate font-mono text-[11px] text-hueso/90">
                    {project.baseBranch}
                  </p>
                </div>
                <div className="min-w-0">
                  <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
                    Verify
                  </p>
                  <p className="mt-1 truncate font-mono text-[11px] text-hueso/90">
                    {project.verifyCommand}
                  </p>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}

      <p className="mt-6 flex items-start gap-2 text-pretty text-[11px] leading-relaxed text-ceniza/70">
        <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-vermilion" />
        Oni executes the agent and verification commands on your machine. Only
        connect repositories you trust, and keep the daemon off the public
        internet.
      </p>

      <button
        type="button"
        onClick={openCreate}
        aria-label="Add project"
        className="fab-above-nav fixed right-4 z-40 flex h-14 w-14 items-center justify-center rounded-full bg-kintsugi text-sumi transition-colors hover:bg-[#d4ad3a] md:hidden"
      >
        <PlusIcon className="h-6 w-6" />
      </button>

      <ProjectSheet
        key={editingProject?.id ?? "new"}
        open={sheetOpen}
        project={editingProject}
        onClose={closeSheet}
      />

      <ConfirmDialog
        open={Boolean(deletingProject)}
        onClose={() => setDeletingProject(null)}
        onConfirm={() => {
          if (deletingProject) deleteProject.mutate(deletingProject);
        }}
        title="Delete this project?"
        description="Existing tasks keep their history and worktrees. New tasks will not be able to use this repository until you connect it again."
        confirmLabel="Delete project"
        tone="danger"
        loading={deleteProject.isPending}
      >
        <div className="rounded-lg border border-vermilion/20 bg-vermilion/5 px-3 py-2">
          <p className="font-mono text-[11px] text-vermilion">
            {deletingProject?.name}
          </p>
          <p className="mt-1 truncate font-mono text-[10px] text-vermilion/75">
            {deletingProject?.path}
          </p>
        </div>
      </ConfirmDialog>
    </div>
  );
}

function ProjectSheet({
  open,
  project,
  onClose,
}: {
  open: boolean;
  project: Project | null;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [baseBranch, setBaseBranch] = useState("main");
  const [verifyCommand, setVerifyCommand] = useState("npm test");
  const [agent, setAgent] = useState<AgentPreset>("opencode");
  const [customCommand, setCustomCommand] = useState("");
  const [error, setError] = useState<string | null>(null);
  const initialized = useRef(false);

  useEffect(() => {
    if (!open) {
      initialized.current = false;
      return;
    }
    if (initialized.current) return;
    initialized.current = true;
    setName(project?.name ?? "");
    setPath(project?.path ?? "");
    setBaseBranch(project?.baseBranch ?? "main");
    setVerifyCommand(project?.verifyCommand ?? "npm test");
    setAgent(project?.agent ?? "opencode");
    setCustomCommand(project?.customCommand ?? "");
    setError(null);
  }, [open, project]);

  const saveProject = useMutation({
    mutationFn: (input: NewProjectInput) =>
      project
        ? api.updateProject(project.id, input)
        : api.createProject(input),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      toast(project ? "Project updated" : "Repository connected", "success");
      onClose();
    },
    onError: (mutationError: Error) => setError(mutationError.message),
  });

  const canSubmit =
    Boolean(name.trim()) &&
    Boolean(path.trim()) &&
    Boolean(verifyCommand.trim()) &&
    (agent !== "custom" || Boolean(customCommand.trim()));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    if (!name.trim() || !path.trim() || !verifyCommand.trim()) {
      setError("Name, path and verification command are required.");
      return;
    }
    if (agent === "custom" && !customCommand.trim()) {
      setError("A custom agent needs a command.");
      return;
    }
    saveProject.mutate({
      name: name.trim(),
      path: path.trim(),
      baseBranch: baseBranch.trim() || "main",
      verifyCommand: verifyCommand.trim(),
      agent,
      customCommand: agent === "custom" ? customCommand.trim() : null,
    });
  };

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={project ? "Edit project" : "Connect a repository"}
      description={
        project
          ? "Update the repository configuration. The path and base branch are validated again."
          : "Use an absolute path on the machine running the daemon. Oni validates the path and base branch before saving."
      }
      footer={
        <div>
          {error ? (
            <p className="mb-3 rounded-lg border border-vermilion/30 bg-vermilion/5 px-3 py-2 text-xs leading-relaxed text-vermilion">
              {error}
            </p>
          ) : null}
          <Button
            type="submit"
            form="project-form"
            variant="primary"
            className="w-full"
            loading={saveProject.isPending}
            disabled={!canSubmit}
          >
            {project ? "Save changes" : "Connect repository"}
          </Button>
        </div>
      }
    >
      <form id="project-form" onSubmit={submit} className="space-y-5">
        <Field
          label="Repository path"
          htmlFor="project-path"
          hint="Example: C:/code/my-app or /Users/you/code/my-app"
        >
          <input
            id="project-path"
            className={`${inputClass} font-mono text-[13px]`}
            value={path}
            onChange={(event) => {
              setPath(event.target.value);
              if (!name) {
                const guess = event.target.value
                  .replace(/[\\/]+$/, "")
                  .split(/[\\/]/)
                  .pop();
                if (guess) setName(guess);
              }
            }}
            placeholder="C:\code\my-app"
            autoFocus
          />
        </Field>

        <div className="grid gap-5 sm:grid-cols-2">
          <Field label="Name" htmlFor="project-name">
            <input
              id="project-name"
              className={inputClass}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="my-app"
            />
          </Field>
          <Field label="Base branch" htmlFor="project-branch">
            <input
              id="project-branch"
              className={`${inputClass} font-mono text-[13px]`}
              value={baseBranch}
              onChange={(event) => setBaseBranch(event.target.value)}
              placeholder="main"
            />
          </Field>
        </div>

        <Field
          label="Verification command"
          htmlFor="project-verify"
          hint="Runs inside the worktree after every agent attempt. Use the command that proves the change is good."
        >
          <input
            id="project-verify"
            className={`${inputClass} font-mono text-[13px]`}
            value={verifyCommand}
            onChange={(event) => setVerifyCommand(event.target.value)}
            placeholder="npm test"
          />
        </Field>

        <Field label="Default agent" htmlFor="project-agent">
          <div className="relative">
            <select
              id="project-agent"
              className={selectClass}
              value={agent}
              onChange={(event) => setAgent(event.target.value as AgentPreset)}
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

        {agent === "custom" ? (
          <Field
            label="Custom command"
            htmlFor="project-custom"
            hint="Executed in the worktree with the task prompt on stdin."
          >
            <textarea
              id="project-custom"
              className={`${textareaClass} font-mono text-[13px]`}
              value={customCommand}
              onChange={(event) => setCustomCommand(event.target.value)}
              placeholder="my-agent --headless"
            />
          </Field>
        ) : null}
      </form>
    </Sheet>
  );
}
