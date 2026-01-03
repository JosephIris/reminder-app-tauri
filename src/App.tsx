import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";

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
import { useReminders } from "./hooks/useReminders";
import type { Reminder } from "./types";

function App() {
  const {
    pending,
    completed,
    loading,
    syncing,
    leavingIds,
    addReminder,
    completeReminder,
    deleteReminder,
    snoozeReminder,
    updateReminder,
    refresh,
    refreshFromCloud,
    reorderReminders,
  } = useReminders();

  const [editingReminder, setEditingReminder] = useState<Reminder | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [focusedReminderId, setFocusedReminderId] = useState<number | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<{ version: string; download: () => Promise<void> } | null>(null);
  const [updating, setUpdating] = useState(false);
  const [checkingForUpdates, setCheckingForUpdates] = useState(false);
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Track if bar is currently shown
  const barShownRef = useRef(false);

  // Track if we're the source of the focus event (to avoid processing our own emit)
  const isLocalFocusRef = useRef(false);

  // Handle focus from list - emit to bar
  const handleFocusReminder = useCallback(async (id: number | null) => {
    setFocusedReminderId(id);
    // Mark that we're emitting, so we ignore our own event
    isLocalFocusRef.current = true;
    // Emit to bar so it syncs
    await emit("focus-reminder", { id });
  }, []);

  // Drag and drop handlers
  const handleDragStart = useCallback((e: React.DragEvent, index: number) => {
    setDraggedIndex(index);
    e.dataTransfer.effectAllowed = 'move';
    // Add some drag image styling
    if (e.target instanceof HTMLElement) {
      e.dataTransfer.setDragImage(e.target, 0, 0);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault(); // Required to allow drop
    e.dataTransfer.dropEffect = 'move';
    if (draggedIndex !== null) {
      setDragOverIndex(index);
    }
  }, [draggedIndex]);

  const handleDrop = useCallback(async (e: React.DragEvent, dropIndex: number) => {
    e.preventDefault();
    if (draggedIndex !== null && draggedIndex !== dropIndex) {
      // Calculate new order
      const newPending = [...pending];
      const [draggedItem] = newPending.splice(draggedIndex, 1);
      newPending.splice(dropIndex, 0, draggedItem);

      // Get ordered IDs and save
      const orderedIds = newPending.map(r => r.id);
      await reorderReminders(orderedIds);
    }

    setDraggedIndex(null);
    setDragOverIndex(null);
  }, [draggedIndex, pending, reorderReminders]);

  const handleDragEnd = useCallback(() => {
    setDraggedIndex(null);
    setDragOverIndex(null);
  }, []);

  // Check for due reminders every 10 seconds
  useEffect(() => {
    // Don't check while still loading initial data
    if (loading) return;

    const checkDueReminders = async () => {
      const now = new Date();

      // Collect all due reminders
      const dueReminders = pending.filter((reminder) => {
        if (reminder.is_completed) return false;
        const dueTime = new Date(reminder.due_time);
        return dueTime <= now;
      });

      if (dueReminders.length > 0) {
        // Show bar if not already shown
        if (!barShownRef.current) {
          try {
            await invoke("show_reminder_bar");
            barShownRef.current = true;
            // Bar will fetch its own reminders on load
          } catch (e) {
            console.error("Failed to show reminder bar:", e);
          }
        }
        // Bar listens for refresh-reminders and fetches its own data
      } else if (barShownRef.current) {
        // Hide bar when no reminders due
        try {
          await invoke("hide_reminder_bar");
          barShownRef.current = false;
        } catch (e) {
          console.error("Failed to hide reminder bar:", e);
        }
      }
    };

    // Check immediately
    checkDueReminders();

    // Set up interval - check every 5 seconds
    const intervalId = setInterval(checkDueReminders, 5000);
    return () => clearInterval(intervalId);
  }, [pending, loading]);

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
              await invoke("install_update", { downloadUrl: update.download_url });
              // App will restart automatically after self-replace
              await relaunch();
            } catch (e) {
              console.error("Update failed:", e);
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
      // Ignore if we're the source of this event
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

  // Bar now fetches its own reminders directly from Rust backend on load,
  // and listens for "refresh-reminders" events for updates

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

        {/* Input */}
        <ReminderInput onAdd={addReminder} syncing={syncing} inputRef={inputRef} />

        {/* Pending reminders - px-1 gives room for glow effect */}
        <div className="flex-1 overflow-y-auto mt-4 space-y-2 px-1">
          {pending.length === 0 ? (
            <div className="text-center py-12">
              <div className="text-4xl mb-3 opacity-50">📭</div>
              <p className="text-gray-500 text-sm">No upcoming reminders</p>
              <p className="text-gray-600 text-xs mt-1">Add one above to get started</p>
            </div>
          ) : (
            <>
              <div className="flex items-center justify-between px-1 mb-2">
                <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Upcoming
                </h2>
                <span className="text-xs text-gray-600">{pending.length} reminder{pending.length !== 1 ? 's' : ''}</span>
              </div>
              {pending.map((reminder, index) => (
                <ReminderItem
                  key={reminder.id}
                  reminder={reminder}
                  index={index}
                  isFocused={focusedReminderId === reminder.id}
                  isLeaving={leavingIds.has(reminder.id)}
                  isDragging={draggedIndex === index}
                  isDragOver={dragOverIndex === index}
                  onComplete={completeReminder}
                  onDelete={deleteReminder}
                  onSnooze={snoozeReminder}
                  onEdit={setEditingReminder}
                  onFocus={handleFocusReminder}
                  onDragStart={handleDragStart}
                  onDragOver={handleDragOver}
                  onDrop={handleDrop}
                  onDragEnd={handleDragEnd}
                />
              ))}
            </>
          )}

          {/* Completed section */}
          <CompletedSection reminders={completed} onDelete={deleteReminder} />
        </div>
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
    </div>
  );
}

export default App;
