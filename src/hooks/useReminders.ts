import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Reminder } from "../types";

export function useReminders() {
  const [pending, setPending] = useState<Reminder[]>([]);
  const [completed, setCompleted] = useState<Reminder[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);

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
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const completeReminder = useCallback(async (id: number) => {
    setSyncing(true);
    try {
      await invoke("complete_reminder", { id });
      await refresh();
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const deleteReminder = useCallback(async (id: number) => {
    setSyncing(true);
    try {
      await invoke("delete_reminder", { id });
      await refresh();
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const snoozeReminder = useCallback(async (id: number, minutes: number) => {
    setSyncing(true);
    try {
      await invoke("snooze_reminder", { id, minutes });
      await refresh();
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
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  const refreshFromCloud = useCallback(async () => {
    setSyncing(true);
    try {
      const synced = await invoke<boolean>("refresh_from_cloud");
      await refresh();
      return synced;
    } finally {
      setSyncing(false);
    }
  }, [refresh]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return {
    pending,
    completed,
    loading,
    syncing,
    refresh,
    addReminder,
    completeReminder,
    deleteReminder,
    snoozeReminder,
    updateReminder,
    refreshFromCloud,
  };
}
