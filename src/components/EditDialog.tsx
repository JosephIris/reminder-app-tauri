import { useState, useEffect, useRef } from "react";
import type { Reminder, RecurrenceType } from "../types";

interface EditDialogProps {
  reminder: Reminder;
  onSave: (id: number, message: string, dueTime: Date, recurrence: string) => Promise<void>;
  onClose: () => void;
}

export function EditDialog({ reminder, onSave, onClose }: EditDialogProps) {
  const [message, setMessage] = useState(reminder.message);
  const [date, setDate] = useState("");
  const [time, setTime] = useState("");
  const [recurrence, setRecurrence] = useState<RecurrenceType>(reminder.recurrence);
  const [saving, setSaving] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const dueTime = new Date(reminder.due_time);
    setDate(dueTime.toISOString().split("T")[0]);
    setTime(dueTime.toTimeString().slice(0, 5));
  }, [reminder]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [onClose]);

  const handleSave = async () => {
    if (!message.trim() || saving) return;

    setSaving(true);
    try {
      const dueTime = new Date(`${date}T${time}`);
      await onSave(reminder.id, message, dueTime, recurrence);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-fade-in">
      <div
        ref={dialogRef}
        className="bg-dark-800 rounded-xl p-6 w-full max-w-md shadow-2xl animate-slide-up"
      >
        <h2 className="text-lg font-semibold text-white mb-4">Edit Reminder</h2>

        <div className="space-y-4">
          {/* Message */}
          <div>
            <label className="block text-sm text-gray-400 mb-1">Message</label>
            <input
              type="text"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              className="w-full bg-dark-700 border border-dark-500 rounded-lg px-3 py-2
                         text-white focus:outline-none focus:border-accent-blue"
            />
          </div>

          {/* Date & Time */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-sm text-gray-400 mb-1">Date</label>
              <input
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
                className="w-full bg-dark-700 border border-dark-500 rounded-lg px-3 py-2
                           text-white focus:outline-none focus:border-accent-blue"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">Time</label>
              <input
                type="time"
                value={time}
                onChange={(e) => setTime(e.target.value)}
                className="w-full bg-dark-700 border border-dark-500 rounded-lg px-3 py-2
                           text-white focus:outline-none focus:border-accent-blue"
              />
            </div>
          </div>

          {/* Recurrence */}
          <div>
            <label className="block text-sm text-gray-400 mb-1">Repeat</label>
            <select
              value={recurrence}
              onChange={(e) => setRecurrence(e.target.value as RecurrenceType)}
              className="w-full bg-dark-700 border border-dark-500 rounded-lg px-3 py-2
                         text-white focus:outline-none focus:border-accent-blue"
            >
              <option value="none">Never</option>
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
            </select>
          </div>
        </div>

        {/* Actions */}
        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={onClose}
            className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !message.trim()}
            className="px-4 py-2 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600
                       text-white rounded-lg transition-colors"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
