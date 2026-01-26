export type UrgencyType = "now" | "today" | "soon" | "whenever";
export type ListType = "actual" | "backlog";

export interface Reminder {
  id: number;
  message: string;
  urgency: UrgencyType;
  list_type: ListType;
  created_at: string; // ISO string
  is_completed: boolean;
  completed_at?: string; // ISO string
  sort_order: number;
}

export interface ReminderStore {
  pending: Reminder[];
  completed: Reminder[];
}
