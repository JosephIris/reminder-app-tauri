import { useState, useEffect, useCallback } from "react";

export type ToastType = "success" | "error" | "info";

interface Toast {
  id: number;
  message: string;
  type: ToastType;
  undoAction?: () => void;
  duration: number;
}

let toastId = 0;
let toastListeners: ((toast: Toast) => void)[] = [];

export function showToast(message: string, type: ToastType = "info", undoAction?: () => void) {
  const toast: Toast = {
    id: ++toastId,
    message,
    type,
    undoAction,
    duration: undoAction ? 5000 : 3000, // Longer duration if has undo
  };
  toastListeners.forEach((listener) => listener(toast));
}

export function ToastContainer() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [progress, setProgress] = useState<Record<number, number>>({});

  useEffect(() => {
    const listener = (toast: Toast) => {
      setToasts((prev) => [...prev, toast]);
      setProgress((prev) => ({ ...prev, [toast.id]: 100 }));

      // Progress countdown
      const startTime = Date.now();
      const interval = setInterval(() => {
        const elapsed = Date.now() - startTime;
        const remaining = Math.max(0, 100 - (elapsed / toast.duration) * 100);
        setProgress((prev) => ({ ...prev, [toast.id]: remaining }));

        if (remaining <= 0) {
          clearInterval(interval);
          setToasts((prev) => prev.filter((t) => t.id !== toast.id));
          setProgress((prev) => {
            const { [toast.id]: _, ...rest } = prev;
            return rest;
          });
        }
      }, 50);
    };

    toastListeners.push(listener);
    return () => {
      toastListeners = toastListeners.filter((l) => l !== listener);
    };
  }, []);

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    setProgress((prev) => {
      const { [id]: _, ...rest } = prev;
      return rest;
    });
  }, []);

  const handleUndo = useCallback((toast: Toast) => {
    if (toast.undoAction) {
      toast.undoAction();
    }
    removeToast(toast.id);
  }, [removeToast]);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`
            relative overflow-hidden
            px-4 py-3 rounded-lg shadow-lg
            transform transition-all duration-300 ease-out
            animate-slide-in min-w-[280px]
            ${toast.type === "success" ? "bg-green-600/90 text-white" : ""}
            ${toast.type === "error" ? "bg-red-600/90 text-white" : ""}
            ${toast.type === "info" ? "bg-dark-700/95 text-white border border-dark-600" : ""}
          `}
        >
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              {toast.type === "success" && <span className="text-green-200">✓</span>}
              {toast.type === "error" && <span className="text-red-200">✕</span>}
              {toast.type === "info" && <span className="text-blue-300">ℹ</span>}
              <span className="text-sm">{toast.message}</span>
            </div>

            <div className="flex items-center gap-2">
              {toast.undoAction && (
                <button
                  onClick={() => handleUndo(toast)}
                  className="px-2 py-1 text-xs font-medium bg-white/20 hover:bg-white/30 rounded transition-colors"
                >
                  Undo
                </button>
              )}
              <button
                onClick={() => removeToast(toast.id)}
                className="text-white/60 hover:text-white/90 transition-colors"
              >
                ✕
              </button>
            </div>
          </div>

          {/* Progress bar */}
          {toast.undoAction && (
            <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-black/20">
              <div
                className="h-full bg-white/40 transition-all duration-50"
                style={{ width: `${progress[toast.id] || 0}%` }}
              />
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
