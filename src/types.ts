export type RecurrenceType = "none" | "daily" | "weekly";

export interface Reminder {
  id: number;
  message: string;
  due_time: string; // ISO string
  created_at: string; // ISO string
  recurrence: RecurrenceType;
  is_completed: boolean;
  is_snoozed: boolean;
  original_due_time?: string; // ISO string
  completed_at?: string; // ISO string
}

export interface ReminderStore {
  pending: Reminder[];
  completed: Reminder[];
}
