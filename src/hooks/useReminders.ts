import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import type { Reminder, UrgencyType, ListType } from "../types";
import { showToast } from "../components/Toast";

export function useReminders() {
  const [pending, setPending] = useState<Reminder[]>([]);
  const [completed, setCompleted] = useState<Reminder[]>([]);
  const [stats, setStats] = useState<{ today: number; week: number }>({ today: 0, week: 0 });
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [leavingIds, setLeavingIds] = useState<Set<number>>(new Set());

  // Refs for stable callback access to current state
  const pendingRef = useRef(pending);
  const completedRef = useRef(completed);
  pendingRef.current = pending;
  completedRef.current = completed;

  // Derived state: actual and backlog lists
  const actual = useMemo(() =>
    pending.filter(r => r.list_type === "actual").sort((a, b) => a.sort_order - b.sort_order),
    [pending]
  );

  const backlog = useMemo(() =>
    pending.filter(r => r.list_type === "backlog").sort((a, b) => a.sort_order - b.sort_order),
    [pending]
  );

  const refresh = useCallback(async () => {
    try {
      const [pendingList, completedList, statsResult] = await Promise.all([
        invoke<Reminder[]>("get_pending_reminders"),
        invoke<Reminder[]>("get_completed_reminders"),
        invoke<[number, number]>("get_completion_stats"),
      ]);
      setPending(pendingList);
      setCompleted(completedList);
      setStats({ today: statsResult[0], week: statsResult[1] });
    } catch (error) {
      console.error("Failed to fetch reminders:", error);
      showToast("Failed to load reminders", "error");
    } finally {
      setLoading(false);
    }
  }, []);

  const addReminder = useCallback((
    message: string,
    urgency: UrgencyType = "today",
    listType: ListType = "actual"
  ) => {
    // Optimistic update with temp ID
    const tempId = -Date.now();
    const tempReminder: Reminder = {
      id: tempId,
      message,
      urgency,
      list_type: listType,
      is_completed: false,
      created_at: new Date().toISOString(),
      sort_order: listType === "actual" ? -1 : 9999,
    };
    setPending(prev => [tempReminder, ...prev]);
    showToast("Task added", "success");

    // Persist in background - NO refresh to avoid overwriting other optimistic updates
    invoke("add_reminder", { message, urgency, listType })
      .then(() => emit("refresh-reminders"))
      .then(() => invoke("sync_to_cloud_background"))
      .catch((error) => {
        console.error("Failed to add reminder:", error);
        showToast("Failed to add task", "error");
        setPending(prev => prev.filter(r => r.id !== tempId)); // Remove temp
      });
  }, []);

  const completeReminder = useCallback((id: number) => {
    // Find the reminder using ref for current state
    const reminder = pendingRef.current.find(r => r.id === id);
    if (!reminder) return;

    // Start leaving animation
    setLeavingIds(prev => new Set(prev).add(id));

    // Optimistic update after short delay for animation
    setTimeout(() => {
      setPending(prev => prev.filter(r => r.id !== id));
      setCompleted(prev => [{
        ...reminder,
        is_completed: true,
        completed_at: new Date().toISOString(),
      }, ...prev]);
      setLeavingIds(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
      // Update stats optimistically
      setStats(prev => ({ today: prev.today + 1, week: prev.week + 1 }));
    }, 300);

    // Show toast with undo immediately
    showToast("Completed", "success", async () => {
      try {
        await invoke("uncomplete_reminder", { id });
        await refresh();
        await emit("refresh-reminders");
        showToast("Restored", "info");
      } catch (e) {
        console.error("Failed to undo:", e);
      }
    });

    // Persist in background
    invoke("complete_reminder", { id })
      .then(() => emit("refresh-reminders"))
      .then(() => invoke("sync_to_cloud_background"))
      .catch((error) => {
        console.error("Failed to complete reminder:", error);
        showToast("Failed to complete task", "error");
        refresh(); // Revert on error
      });
  }, [refresh]);

  const deleteReminder = useCallback((id: number, skipAnimation = false) => {
    // Find the reminder using refs for current state
    const reminder = pendingRef.current.find(r => r.id === id) || completedRef.current.find(r => r.id === id);

    if (!skipAnimation) {
      // Start leaving animation
      setLeavingIds(prev => new Set(prev).add(id));

      // Optimistic update after animation
      setTimeout(() => {
        setPending(prev => prev.filter(r => r.id !== id));
        setCompleted(prev => prev.filter(r => r.id !== id));
        setLeavingIds(prev => {
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
      }, 300);
    } else {
      // Immediate optimistic update
      setPending(prev => prev.filter(r => r.id !== id));
      setCompleted(prev => prev.filter(r => r.id !== id));
    }

    // Show toast with undo option immediately
    if (reminder) {
      showToast("Deleted", "info", async () => {
        try {
          await invoke("add_reminder", {
            message: reminder.message,
            urgency: reminder.urgency,
            listType: reminder.list_type,
          });
          await refresh();
          await emit("refresh-reminders");
          showToast("Restored", "info");
        } catch (e) {
          console.error("Failed to undo:", e);
        }
      });
    }

    // Persist in background
    invoke("delete_reminder", { id })
      .then(() => emit("refresh-reminders"))
      .then(() => invoke("sync_to_cloud_background"))
      .catch((error) => {
        console.error("Failed to delete reminder:", error);
        showToast("Failed to delete task", "error");
        refresh(); // Revert on error
      });
  }, [refresh]);

  const updateReminder = useCallback(async (id: number, message: string, urgency: UrgencyType) => {
    setSyncing(true);
    try {
      await invoke("update_reminder", {
        id,
        message,
        urgency,
      });
      await refresh();
      await emit("refresh-reminders");
      showToast("Task updated", "success");
    } catch (error) {
      console.error("Failed to update reminder:", error);
      showToast("Failed to update task", "error");
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const moveReminder = useCallback((id: number, toList: ListType) => {
    // Optimistic update: move locally first for instant feedback
    setPending(prev => {
      const reminder = prev.find(r => r.id === id);
      if (!reminder || reminder.list_type === toList) return prev;

      return prev.map(r => {
        if (r.id === id) {
          return { ...r, list_type: toList, sort_order: toList === "actual" ? -1 : 9999 };
        }
        return r;
      });
    });

    // Persist in background - fire and forget for snappy UX
    invoke("move_reminder", { id, toList })
      .then(() => emit("refresh-reminders"))
      .then(() => invoke("sync_to_cloud_background"))
      .catch((error) => {
        console.error("Failed to move reminder:", error);
        showToast("Failed to move task", "error");
        refresh(); // Revert on error
      });
  }, [refresh]);

  const setUrgency = useCallback((id: number, urgency: UrgencyType) => {
    // Optimistic update: update locally first for instant feedback
    setPending(prev => prev.map(r => r.id === id ? { ...r, urgency } : r));

    // Persist in background - fire and forget for snappy UX
    invoke("set_urgency", { id, urgency })
      .then(() => emit("refresh-reminders"))
      .then(() => invoke("sync_to_cloud_background"))
      .catch((error) => {
        console.error("Failed to set urgency:", error);
        showToast("Failed to update urgency", "error");
        refresh(); // Revert on error
      });
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
      // Add any reminders not in orderedIds (shouldn't happen, but just in case)
      for (const r of prev) {
        if (!orderedIds.includes(r.id)) {
          reordered.push(r);
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

  // Periodic cloud sync every 5 minutes
  useEffect(() => {
    const SYNC_INTERVAL = 5 * 60 * 1000; // 5 minutes

    const periodicSync = async () => {
      try {
        const synced = await invoke<boolean>("refresh_from_cloud");
        if (synced) {
          await refresh();
          console.log("Periodic sync completed");
        }
      } catch (e) {
        console.log("Periodic sync skipped:", e);
      }
    };

    const interval = setInterval(periodicSync, SYNC_INTERVAL);
    return () => clearInterval(interval);
  }, [refresh]);

  return {
    pending,
    actual,
    backlog,
    completed,
    stats,
    loading,
    syncing,
    leavingIds,
    refresh,
    addReminder,
    completeReminder,
    deleteReminder,
    updateReminder,
    moveReminder,
    setUrgency,
    refreshFromCloud,
    reorderReminders,
  };
}
