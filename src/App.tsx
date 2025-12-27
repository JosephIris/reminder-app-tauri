import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { TitleBar } from "./components/TitleBar";
import { ReminderInput } from "./components/ReminderInput";
import { ReminderItem } from "./components/ReminderItem";
import { CompletedSection } from "./components/CompletedSection";
import { EditDialog } from "./components/EditDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { useReminders } from "./hooks/useReminders";
import type { Reminder } from "./types";

function App() {
  const {
    pending,
    completed,
    loading,
    syncing,
    addReminder,
    completeReminder,
    deleteReminder,
    snoozeReminder,
    updateReminder,
    refresh,
    refreshFromCloud,
  } = useReminders();

  const [editingReminder, setEditingReminder] = useState<Reminder | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Track if bar is currently shown
  const barShownRef = useRef(false);

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

    // Set up interval
    const intervalId = setInterval(checkDueReminders, 10000);
    return () => clearInterval(intervalId);
  }, [pending, loading]);

  // Listen for refresh events from Rust backend
  useEffect(() => {
    const unlisten = listen("refresh-reminders", () => {
      refresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // Listen for focus events (when window is shown)
  useEffect(() => {
    const unlisten = listen("tauri://focus", () => {
      refresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // Listen for focus-input event (from global hotkey / tray quick add)
  useEffect(() => {
    const unlisten = listen("focus-input", () => {
      inputRef.current?.focus();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Bar now fetches its own reminders directly from Rust backend on load,
  // and listens for "refresh-reminders" events for updates

  if (loading) {
    return (
      <div className="h-screen flex flex-col bg-dark-900">
        <TitleBar onSettingsClick={() => setShowSettings(true)} />
        <div className="flex-1 flex items-center justify-center">
          <div className="text-gray-400 animate-pulse-soft">Loading...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-dark-900">
      <TitleBar onSettingsClick={() => setShowSettings(true)} />

      <div className="flex-1 overflow-hidden flex flex-col p-4">
        {/* Input */}
        <ReminderInput onAdd={addReminder} syncing={syncing} inputRef={inputRef} />

        {/* Pending reminders */}
        <div className="flex-1 overflow-y-auto mt-3 space-y-1">
          {pending.length === 0 ? (
            <div className="text-center text-gray-600 py-8 text-sm">
              No reminders
            </div>
          ) : (
            pending.map((reminder) => (
              <ReminderItem
                key={reminder.id}
                reminder={reminder}
                onComplete={completeReminder}
                onDelete={deleteReminder}
                onSnooze={snoozeReminder}
                onEdit={setEditingReminder}
              />
            ))
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
        />
      )}
    </div>
  );
}

export default App;
