import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import type { Reminder } from "../types";
import { showToast } from "../components/Toast";

export function useReminders() {
  const [pending, setPending] = useState<Reminder[]>([]);
  const [completed, setCompleted] = useState<Reminder[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [leavingIds, setLeavingIds] = useState<Set<number>>(new Set());

  // Store last deleted/completed for undo
  const lastActionRef = useRef<{ type: "complete" | "delete"; reminder: Reminder } | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [pendingList, completedList] = await Promise.all([
        invoke<Reminder[]>("get_pending_reminders"),
        invoke<Reminder[]>("get_completed_reminders"),
      ]);
      setPending(pendingList);
      setCompleted(completedList);
    } catch (error) {
      console.error("Failed to fetch reminders:", error);
      showToast("Failed to load reminders", "error");
    } finally {
      setLoading(false);
    }
  }, []);

  const addReminder = useCallback(async (message: string, dueTime: Date, recurrence: string = "none") => {
    setSyncing(true);
    try {
      await invoke("add_reminder", {
        message,
        dueTime: dueTime.toISOString(),
        recurrence,
      });
      await refresh();
      await emit("refresh-reminders");
      showToast("Reminder added", "success");
    } catch (error) {
      console.error("Failed to add reminder:", error);
      showToast("Failed to add reminder", "error");
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const completeReminder = useCallback(async (id: number) => {
    // Find the reminder before completing for undo
    const reminder = pending.find(r => r.id === id);
    if (!reminder) return;

    // Start leaving animation
    setLeavingIds(prev => new Set(prev).add(id));

    // Wait for animation
    await new Promise(resolve => setTimeout(resolve, 300));

    setSyncing(true);
    try {
      await invoke("complete_reminder", { id });
      lastActionRef.current = { type: "complete", reminder };
      await refresh();
      await emit("refresh-reminders");

      // Show toast with undo option
      showToast("Completed", "success", async () => {
        // Undo: uncomplete the reminder
        try {
          await invoke("uncomplete_reminder", { id });
          await refresh();
          await emit("refresh-reminders");
          showToast("Restored", "info");
        } catch (e) {
          console.error("Failed to undo:", e);
        }
      });
    } catch (error) {
      console.error("Failed to complete reminder:", error);
      showToast("Failed to complete reminder", "error");
    } finally {
      setSyncing(false);
      setLeavingIds(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }, [refresh, pending]);

  const deleteReminder = useCallback(async (id: number, skipAnimation = false) => {
    // Find the reminder before deleting for potential restore
    const reminder = pending.find(r => r.id === id) || completed.find(r => r.id === id);

    if (!skipAnimation) {
      // Start leaving animation
      setLeavingIds(prev => new Set(prev).add(id));
      // Wait for animation
      await new Promise(resolve => setTimeout(resolve, 300));
    }

    setSyncing(true);
    try {
      await invoke("delete_reminder", { id });
      if (reminder) {
        lastActionRef.current = { type: "delete", reminder };
      }
      await refresh();
      await emit("refresh-reminders");

      // Show toast with undo option (only if we have the reminder data)
      if (reminder) {
        showToast("Deleted", "info", async () => {
          // Undo: re-add the reminder
          try {
            await invoke("add_reminder", {
              message: reminder.message,
              dueTime: reminder.due_time,
              recurrence: reminder.recurrence || "none",
            });
            await refresh();
            await emit("refresh-reminders");
            showToast("Restored", "info");
          } catch (e) {
            console.error("Failed to undo:", e);
          }
        });
      }
    } catch (error) {
      console.error("Failed to delete reminder:", error);
      showToast("Failed to delete reminder", "error");
    } finally {
      setSyncing(false);
      setLeavingIds(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }, [refresh, pending, completed]);

  const snoozeReminder = useCallback(async (id: number, minutes: number) => {
    setSyncing(true);
    try {
      await invoke("snooze_reminder", { id, minutes });
      await refresh();
      await emit("refresh-reminders");
      showToast(`Snoozed for ${minutes} minutes`, "info");
    } catch (error) {
      console.error("Failed to snooze reminder:", error);
      showToast("Failed to snooze reminder", "error");
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const updateReminder = useCallback(async (id: number, message: string, dueTime: Date, recurrence: string) => {
    setSyncing(true);
    try {
      await invoke("update_reminder", {
        id,
        message,
        dueTime: dueTime.toISOString(),
        recurrence,
      });
      await refresh();
      await emit("refresh-reminders");
      showToast("Reminder updated", "success");
    } catch (error) {
      console.error("Failed to update reminder:", error);
      showToast("Failed to update reminder", "error");
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const refreshFromCloud = useCallback(async () => {
    setSyncing(true);
    try {
      const synced = await invoke<boolean>("refresh_from_cloud");
      await refresh();
      if (synced) {
        showToast("Synced from cloud", "success");
      }
      return synced;
    } catch (error) {
      console.error("Failed to sync from cloud:", error);
      showToast("Failed to sync from cloud", "error");
      return false;
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const reorderReminders = useCallback(async (orderedIds: number[]) => {
    // Optimistic update: reorder locally FIRST for instant feedback
    setPending(prev => {
      const idToReminder = new Map(prev.map(r => [r.id, r]));
      const reordered: Reminder[] = [];
      for (const id of orderedIds) {
        const reminder = idToReminder.get(id);
        if (reminder) {
          reordered.push({ ...reminder, sort_order: reordered.length });
        }
      }
      return reordered;
    });

    // Persist locally (fast), then sync to cloud in background
    try {
      await invoke("reorder_reminders", { orderedIds });
      await emit("refresh-reminders");
      // Cloud sync in background - don't await
      invoke("sync_to_cloud_background").catch((e) => {
        console.log("Background cloud sync skipped:", e);
      });
    } catch (error) {
      console.error("Failed to reorder reminders:", error);
      showToast("Failed to save order", "error");
      refresh();
    }
  }, [refresh]);

  // Sync from cloud on startup and load reminders
  useEffect(() => {
    const initAndSync = async () => {
      try {
        // Try to sync from cloud first
        const synced = await invoke<boolean>("sync_on_startup");
        if (synced) {
          showToast("Synced from cloud", "success");
        }
      } catch (e) {
        // Cloud sync failed or not configured - that's OK
        console.log("Startup sync skipped:", e);
      }
      // Load reminders regardless of sync result
      await refresh();
    };
    initAndSync();
  }, [refresh]);

  return {
    pending,
    completed,
    loading,
    syncing,
    leavingIds,
    refresh,
    addReminder,
    completeReminder,
    deleteReminder,
    snoozeReminder,
    updateReminder,
    refreshFromCloud,
    reorderReminders,
  };
}
