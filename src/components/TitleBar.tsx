import { getCurrentWindow } from "@tauri-apps/api/window";

interface TitleBarProps {
  onSettingsClick?: () => void;
}

export function TitleBar({ onSettingsClick }: TitleBarProps) {
  const appWindow = getCurrentWindow();

  const handleDragStart = (e: React.MouseEvent) => {
    // Only start drag if clicking on the drag region itself, not buttons
    if ((e.target as HTMLElement).closest('button')) return;
    appWindow.startDragging();
  };

  return (
    <div
      onMouseDown={handleDragStart}
      className="h-10 flex items-center justify-between px-4 bg-dark-800 select-none cursor-default"
    >
      <div className="flex items-center gap-2">
        <span className="text-lg">⏰</span>
        <span className="font-medium text-white">Reminders</span>
      </div>

      <div className="flex items-center">
        <button
          onClick={onSettingsClick}
          className="p-2 hover:bg-dark-600 rounded transition-colors text-gray-400 hover:text-white"
          title="Settings"
        >
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
        <button
          onClick={() => appWindow.minimize()}
          className="p-2 hover:bg-dark-600 rounded transition-colors text-gray-400 hover:text-white"
          title="Minimize"
        >
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
          </svg>
        </button>
        <button
          onClick={() => appWindow.hide()}
          className="p-2 hover:bg-accent-red rounded transition-colors text-gray-400 hover:text-white"
          title="Close to tray"
        >
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}
