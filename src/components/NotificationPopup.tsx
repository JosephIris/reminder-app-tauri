import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Reminder } from "../types";

interface NotificationPopupProps {
  reminder: Reminder;
  onSnooze: (id: number, minutes: number) => void;
  onComplete: (id: number) => void;
  onDismiss: () => void;
}

export function NotificationPopup({
  reminder,
  onSnooze,
  onComplete,
  onDismiss,
}: NotificationPopupProps) {
  const dueTime = new Date(reminder.due_time);
  const timeStr = dueTime.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });

  // Truncate message if too long
  const displayMessage =
    reminder.message.length > 28
      ? reminder.message.substring(0, 28) + "..."
      : reminder.message;

  useEffect(() => {
    // Play system sound
    const audio = new Audio();
    audio.src = "data:audio/wav;base64,UklGRnoGAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQoGAACBhYqFbF1fdH2JiYqGfXJqYV9ncHuEi4yIgHZsZGJkbXd/hoyMiIF2bGRlZ3B5gYaKiYR7cmljYmVsdX6FiYmFfnNpYWJmbHR9hYmJhX5zaWJjZm10fYaJiYV+c2liY2VsdH2GiYmFfnNpYmNlbHR9homJhX5zaWJjZWx0fYaJiYV+c2liY2VsdH2GiYmFfnNpYmNlbA==";
    audio.play().catch(() => {}); // Ignore if can't play

    // Flash the window
    const appWindow = getCurrentWindow();
    appWindow.requestUserAttention(2); // Critical attention
  }, []);

  return (
    <div className="fixed inset-0 flex items-end justify-center pb-4 pointer-events-none">
      <div
        className="bg-dark-800/90 backdrop-blur-md rounded-xl shadow-2xl p-4 mx-4 max-w-md w-full
                   pointer-events-auto animate-slide-up border border-dark-600"
        style={{ opacity: 0.9 }}
      >
        <div className="flex items-center gap-4">
          {/* Left: Message and time */}
          <div className="flex-1 min-w-0">
            <p className="text-white font-semibold truncate">{displayMessage}</p>
            <p className="text-gray-400 text-sm">{timeStr}</p>
          </div>

          {/* Right: Action buttons */}
          <div className="flex items-center gap-1">
            {/* Snooze 5m */}
            <button
              onClick={() => {
                onSnooze(reminder.id, 5);
                onDismiss();
              }}
              className="px-2 py-1.5 text-xs bg-dark-600 hover:bg-dark-500 text-gray-300 rounded transition-colors"
              title="Snooze 5 minutes"
            >
              +5m
            </button>

            {/* Snooze 15m */}
            <button
              onClick={() => {
                onSnooze(reminder.id, 15);
                onDismiss();
              }}
              className="px-2 py-1.5 text-xs bg-dark-600 hover:bg-dark-500 text-gray-300 rounded transition-colors"
              title="Snooze 15 minutes"
            >
              +15m
            </button>

            {/* Complete */}
            <button
              onClick={() => {
                onComplete(reminder.id);
                onDismiss();
              }}
              className="p-1.5 bg-accent-green/20 hover:bg-accent-green/30 text-accent-green rounded transition-colors"
              title="Complete"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-4 w-4"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </button>

            {/* Dismiss */}
            <button
              onClick={onDismiss}
              className="p-1.5 hover:bg-dark-600 text-gray-400 rounded transition-colors"
              title="Dismiss"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-4 w-4"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
