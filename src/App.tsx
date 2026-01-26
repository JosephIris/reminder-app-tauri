import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";

interface UpdateInfo {
  version: string;
  current_version: string;
  download_url: string;
}
import { TitleBar } from "./components/TitleBar";
import { ReminderInput } from "./components/ReminderInput";
import { ReminderItem } from "./components/ReminderItem";
import { CompletedSection } from "./components/CompletedSection";
import { EditDialog } from "./components/EditDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { ToastContainer } from "./components/Toast";
import { ReportsTab } from "./components/ReportsTab";
import { OrganizePrompt } from "./components/OrganizePrompt";
import { useReminders } from "./hooks/useReminders";
import { useDragReorder } from "./hooks/useDragReorder";
import type { Reminder } from "./types";

type TabType = "tasks" | "reports";

function App() {
  const {
    actual,
    backlog,
    completed,
    stats,
    loading,
    syncing,
    leavingIds,
    addReminder,
    completeReminder,
    deleteReminder,
    updateReminder,
    moveReminder,
    setUrgency,
    refresh,
    refreshFromCloud,
    reorderReminders,
  } = useReminders();

  const [activeTab, setActiveTab] = useState<TabType>("tasks");
  const [editingReminder, setEditingReminder] = useState<Reminder | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [focusedReminderId, setFocusedReminderId] = useState<number | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<{ version: string; download: () => Promise<void> } | null>(null);
  const [updating, setUpdating] = useState(false);
  const [checkingForUpdates, setCheckingForUpdates] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Track if bar is currently shown
  const barShownRef = useRef(false);

  // Track if we're the source of the focus event (to avoid processing our own emit)
  const isLocalFocusRef = useRef(false);

  // Mouse-based drag reorder for actual list
  const handleReorderActual = useCallback(async (fromIndex: number, toIndex: number) => {
    const newActual = [...actual];
    const [draggedItem] = newActual.splice(fromIndex, 1);
    newActual.splice(toIndex, 0, draggedItem);
    const orderedIds = newActual.map(r => r.id);
    await reorderReminders(orderedIds);
  }, [actual, reorderReminders]);

  // Mouse-based drag reorder for backlog list
  const handleReorderBacklog = useCallback(async (fromIndex: number, toIndex: number) => {
    const newBacklog = [...backlog];
    const [draggedItem] = newBacklog.splice(fromIndex, 1);
    newBacklog.splice(toIndex, 0, draggedItem);
    const orderedIds = newBacklog.map(r => r.id);
    await reorderReminders(orderedIds);
  }, [backlog, reorderReminders]);

  const { dragState: dragStateActual, handleMouseDown: handleMouseDownActual, justFinishedDrag: justFinishedDragActual } = useDragReorder({
    onReorder: handleReorderActual,
    itemSelector: '.actual-item',
    containerSelector: '.actual-list-container',
  });

  const { dragState: dragStateBacklog, handleMouseDown: handleMouseDownBacklog, justFinishedDrag: justFinishedDragBacklog } = useDragReorder({
    onReorder: handleReorderBacklog,
    itemSelector: '.backlog-item',
    containerSelector: '.backlog-list-container',
  });

  // Handle focus from list - emit to bar
  const handleFocusReminder = useCallback(async (id: number | null) => {
    // Don't trigger focus if we just finished dragging
    if (justFinishedDragActual.current || justFinishedDragBacklog.current) return;
    setFocusedReminderId(id);
    // Mark that we're emitting, so we ignore our own event
    isLocalFocusRef.current = true;
    // Emit to bar so it syncs
    await emit("focus-reminder", { id });
  }, [justFinishedDragActual, justFinishedDragBacklog]);

  // Show/hide bar based on actual tasks
  useEffect(() => {
    if (loading) return;

    const updateBar = async () => {
      if (actual.length > 0) {
        // Show bar if there are actual tasks
        if (!barShownRef.current) {
          try {
            await invoke("show_reminder_bar");
            barShownRef.current = true;
          } catch (e) {
            console.error("Failed to show reminder bar:", e);
          }
        }
      } else if (barShownRef.current) {
        // Hide bar when no actual tasks
        try {
          await invoke("hide_reminder_bar");
          barShownRef.current = false;
        } catch (e) {
          console.error("Failed to hide reminder bar:", e);
        }
      }
    };

    updateBar();
  }, [actual, loading]);

  // Listen for refresh events from Rust backend
  useEffect(() => {
    const unlisten = listen("refresh-reminders", () => {
      refresh();
    });
    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
    };
  }, [refresh]);

  // Listen for focus events (when window is shown)
  useEffect(() => {
    const unlisten = listen("tauri://focus", () => {
      refresh();
    });
    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
    };
  }, [refresh]);

  // Listen for focus-input event (from global hotkey / tray quick add)
  useEffect(() => {
    const unlisten = listen("focus-input", () => {
      inputRef.current?.focus();
    });
    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
    };
  }, []);

  // Check for updates function - reusable for startup and manual checks
  const checkForUpdates = useCallback(async (): Promise<"available" | "up-to-date" | "error"> => {
    setCheckingForUpdates(true);
    try {
      const update = await invoke<UpdateInfo | null>("check_for_update");
      if (update) {
        console.log(`Update available: ${update.version} (current: ${update.current_version})`);
        setUpdateAvailable({
          version: update.version,
          download: async () => {
            setUpdating(true);
            try {
              console.log("Starting update download from:", update.download_url);
              await invoke("install_update", { downloadUrl: update.download_url });
              console.log("Update installed, exiting for update script to replace exe...");
              await exit(0);
            } catch (e) {
              console.error("Update failed:", e);
              alert(`Update failed: ${e}`);
              setUpdating(false);
            }
          },
        });
        return "available";
      }
      return "up-to-date";
    } catch (e) {
      console.error("Update check failed:", e);
      return "error";
    } finally {
      setCheckingForUpdates(false);
    }
  }, []);

  // Check for updates on startup
  useEffect(() => {
    checkForUpdates();
  }, [checkForUpdates]);

  // Listen for focus-reminder event from the reminder bar
  useEffect(() => {
    const unlisten = listen<{ id: number | null }>("focus-reminder", (event) => {
      if (isLocalFocusRef.current) {
        isLocalFocusRef.current = false;
        return;
      }
      setFocusedReminderId(event.payload.id);
    });
    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
    };
  }, []);

  if (loading) {
    return (
      <div className="h-screen flex flex-col bg-dark-900 rounded-xl border border-dark-700 overflow-hidden">
        <TitleBar onSettingsClick={() => setShowSettings(true)} />
        <div className="flex-1 flex items-center justify-center">
          <div className="text-gray-400 animate-pulse-soft">Loading...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-dark-900 rounded-xl border border-dark-700 overflow-hidden">
      <TitleBar onSettingsClick={() => setShowSettings(true)} />

      <div className="flex-1 overflow-hidden flex flex-col p-4">
        {/* Update banner */}
        {updateAvailable && (
          <div className="mb-3 p-3 bg-accent-blue/20 border border-accent-blue/30 rounded-lg flex items-center justify-between">
            <span className="text-sm text-white">
              Update v{updateAvailable.version} available
            </span>
            <button
              onClick={updateAvailable.download}
              disabled={updating}
              className="px-3 py-1 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 text-white text-sm rounded transition-colors"
            >
              {updating ? "Updating..." : "Update Now"}
            </button>
          </div>
        )}

        {/* Stats bar */}
        <div className="flex items-center justify-between mb-3 px-1">
          <div className="flex gap-4">
            <span className="text-xs text-gray-500">
              <span className="text-accent-green font-medium">{stats.today}</span> today
            </span>
            <span className="text-xs text-gray-500">
              <span className="text-accent-blue font-medium">{stats.week}</span> this week
            </span>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setActiveTab("tasks")}
              className={`px-3 py-1 text-xs rounded transition-colors ${
                activeTab === "tasks"
                  ? "bg-dark-600 text-white"
                  : "text-gray-500 hover:text-gray-300"
              }`}
            >
              Tasks
            </button>
            <button
              onClick={() => setActiveTab("reports")}
              className={`px-3 py-1 text-xs rounded transition-colors ${
                activeTab === "reports"
                  ? "bg-dark-600 text-white"
                  : "text-gray-500 hover:text-gray-300"
              }`}
            >
              Reports
            </button>
          </div>
        </div>

        {activeTab === "tasks" ? (
          <>
            {/* Input */}
            <ReminderInput onAdd={addReminder} syncing={syncing} inputRef={inputRef} />

            {/* Task lists */}
            <div className="flex-1 overflow-y-auto mt-4 space-y-4 px-1">
              {/* Actual section */}
              <div>
                <div className="flex items-center justify-between px-1 mb-2">
                  <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Actual ({actual.length}/6)
                  </h2>
                </div>
                {actual.length === 0 ? (
                  <div className="text-center py-8 flex flex-col items-center justify-center">
                    <div className="text-3xl mb-2 animate-float">✨</div>
                    <p className="text-gray-500 text-xs">No active tasks</p>
                  </div>
                ) : (
                  <div className="space-y-2 actual-list-container">
                    {actual.map((reminder, index) => (
                      <div key={reminder.id} className="actual-item">
                        <ReminderItem
                          reminder={reminder}
                          isFocused={focusedReminderId === reminder.id}
                          isLeaving={leavingIds.has(reminder.id)}
                          isDragging={dragStateActual.draggedIndex === index}
                          isDragOver={dragStateActual.dropTargetIndex === index}
                          onComplete={completeReminder}
                          onDelete={deleteReminder}
                          onEdit={setEditingReminder}
                          onMove={moveReminder}
                          onSetUrgency={setUrgency}
                          onFocus={handleFocusReminder}
                          onMouseDown={(e) => handleMouseDownActual(e, index)}
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Backlog section */}
              {backlog.length > 0 && (
                <div>
                  <div className="flex items-center justify-between px-1 mb-2">
                    <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Backlog ({backlog.length})
                    </h2>
                  </div>
                  <div className="space-y-2 backlog-list-container">
                    {backlog.map((reminder, index) => (
                      <div key={reminder.id} className="backlog-item">
                        <ReminderItem
                          reminder={reminder}
                          isFocused={focusedReminderId === reminder.id}
                          isLeaving={leavingIds.has(reminder.id)}
                          isDragging={dragStateBacklog.draggedIndex === index}
                          isDragOver={dragStateBacklog.dropTargetIndex === index}
                          onComplete={completeReminder}
                          onDelete={deleteReminder}
                          onEdit={setEditingReminder}
                          onMove={moveReminder}
                          onSetUrgency={setUrgency}
                          onFocus={handleFocusReminder}
                          onMouseDown={(e) => handleMouseDownBacklog(e, index)}
                        />
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Completed section */}
              <CompletedSection reminders={completed} onDelete={deleteReminder} />
            </div>
          </>
        ) : (
          <ReportsTab />
        )}
      </div>

      {/* Edit dialog */}
      {editingReminder && (
        <EditDialog
          reminder={editingReminder}
          onSave={updateReminder}
          onClose={() => setEditingReminder(null)}
        />
      )}

      {/* Settings dialog */}
      {showSettings && (
        <SettingsDialog
          onClose={() => setShowSettings(false)}
          onRefreshFromCloud={refreshFromCloud}
          onCheckForUpdates={checkForUpdates}
          checkingForUpdates={checkingForUpdates}
        />
      )}

      {/* Toast notifications */}
      <ToastContainer />

      {/* Organization prompt */}
      <OrganizePrompt
        actualCount={actual.length}
        backlogCount={backlog.length}
        completedToday={stats.today}
        onOpenTasks={() => setActiveTab("tasks")}
      />
    </div>
  );
}

export default App;
