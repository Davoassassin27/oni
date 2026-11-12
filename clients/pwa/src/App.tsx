import { useQuery } from "@tanstack/react-query";
import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell";
import { Toaster } from "./components/ui";
import { api } from "./lib/api";
import { useLiveEvents } from "./lib/useLiveEvents";
import ProjectsPage from "./pages/ProjectsPage";
import SettingsPage from "./pages/SettingsPage";
import TaskDetailPage from "./pages/TaskDetailPage";
import TasksPage from "./pages/TasksPage";

export default function App() {
  const live = useLiveEvents();
  const health = useQuery({
    queryKey: ["health"],
    queryFn: api.health,
    refetchInterval: 20_000,
    retry: false,
  });

  const online =
    live !== null
      ? live
      : health.isSuccess
        ? true
        : health.isError
          ? false
          : null;

  return (
    <>
      <AppShell online={online}>
        <Routes>
          <Route path="/" element={<TasksPage online={online} />} />
          <Route path="/tasks/:id" element={<TaskDetailPage />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AppShell>
      <Toaster />
    </>
  );
}
