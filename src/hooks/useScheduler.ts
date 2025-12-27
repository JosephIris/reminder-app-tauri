import { useEffect, useRef, useCallback } from "react";
import type { Reminder } from "../types";

interface UseSchedulerOptions {
  reminders: Reminder[];
  onReminderDue: (reminder: Reminder) => void;
  checkIntervalMs?: number;
}

export function useScheduler({
  reminders,
  onReminderDue,
  checkIntervalMs = 10000, // Default 10 seconds like Python app
}: UseSchedulerOptions) {
  const firedIdsRef = useRef<Set<number>>(new Set());

  const checkDueReminders = useCallback(() => {
    const now = new Date();

    for (const reminder of reminders) {
      if (reminder.is_completed) continue;
      if (firedIdsRef.current.has(reminder.id)) continue;

      const dueTime = new Date(reminder.due_time);
      if (dueTime <= now) {
        // Reminder is due!
        firedIdsRef.current.add(reminder.id);
        onReminderDue(reminder);

        // Keep only last 100 fired IDs to prevent memory leak
        if (firedIdsRef.current.size > 100) {
          const idsArray = Array.from(firedIdsRef.current);
          firedIdsRef.current = new Set(idsArray.slice(-100));
        }
      }
    }
  }, [reminders, onReminderDue]);

  // Clear fired ID when reminder is snoozed (so it can fire again)
  const clearFiredId = useCallback((id: number) => {
    firedIdsRef.current.delete(id);
  }, []);

  useEffect(() => {
    // Check immediately on mount
    checkDueReminders();

    // Set up interval
    const intervalId = setInterval(checkDueReminders, checkIntervalMs);

    return () => clearInterval(intervalId);
  }, [checkDueReminders, checkIntervalMs]);

  return { clearFiredId };
}
