import { useState, useRef, useCallback, useEffect } from 'react';

interface UseDragReorderOptions {
  onReorder: (fromIndex: number, toIndex: number) => void;
  itemSelector: string;
  containerSelector: string;
}

interface DragState {
  isDragging: boolean;
  draggedIndex: number | null;
  dropTargetIndex: number | null;
}

export function useDragReorder({ onReorder, itemSelector, containerSelector }: UseDragReorderOptions) {
  const [dragState, setDragState] = useState<DragState>({
    isDragging: false,
    draggedIndex: null,
    dropTargetIndex: null,
  });

  const dragStartY = useRef(0);
  const dragOffsetY = useRef(0);
  const ghostRef = useRef<HTMLElement | null>(null);
  const draggedElementRef = useRef<HTMLElement | null>(null);
  const justFinishedDrag = useRef(false);

  const createGhost = useCallback((element: HTMLElement) => {
    const ghost = element.cloneNode(true) as HTMLElement;
    ghost.style.position = 'fixed';
    ghost.style.pointerEvents = 'none';
    ghost.style.zIndex = '10000';
    ghost.style.opacity = '0.9';
    ghost.style.transform = 'rotate(1deg) scale(1.02)';
    ghost.style.boxShadow = '0 8px 24px rgba(0, 0, 0, 0.4)';
    ghost.style.width = `${element.offsetWidth}px`;
    ghost.style.transition = 'none';
    document.body.appendChild(ghost);
    return ghost;
  }, []);

  const removeGhost = useCallback(() => {
    if (ghostRef.current) {
      ghostRef.current.remove();
      ghostRef.current = null;
    }
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent, index: number) => {
    // Don't start drag if clicking on buttons
    if ((e.target as HTMLElement).closest('button')) return;

    const element = e.currentTarget as HTMLElement;
    const rect = element.getBoundingClientRect();

    dragStartY.current = e.clientY;
    dragOffsetY.current = e.clientY - rect.top;
    draggedElementRef.current = element;

    setDragState({
      isDragging: false,
      draggedIndex: index,
      dropTargetIndex: null,
    });
  }, []);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (dragState.draggedIndex === null) return;

    const deltaY = Math.abs(e.clientY - dragStartY.current);

    // Start dragging after 5px movement
    if (!dragState.isDragging && deltaY > 5) {
      if (draggedElementRef.current) {
        ghostRef.current = createGhost(draggedElementRef.current);
        document.body.style.cursor = 'grabbing';
        document.body.classList.add('dragging-active');
      }
      setDragState(prev => ({ ...prev, isDragging: true }));
    }

    if (!dragState.isDragging) return;

    // Update ghost position
    if (ghostRef.current) {
      ghostRef.current.style.left = `${draggedElementRef.current?.getBoundingClientRect().left || 0}px`;
      ghostRef.current.style.top = `${e.clientY - dragOffsetY.current}px`;
    }

    // Find drop target
    const container = document.querySelector(containerSelector);
    if (!container) return;

    const items = Array.from(container.querySelectorAll(itemSelector));
    let newDropIndex = items.length;

    for (let i = 0; i < items.length; i++) {
      const item = items[i] as HTMLElement;
      const rect = item.getBoundingClientRect();
      const midY = rect.top + rect.height / 2;

      if (e.clientY < midY) {
        newDropIndex = i;
        break;
      }
    }

    // Adjust if dragging downward
    if (dragState.draggedIndex !== null && newDropIndex > dragState.draggedIndex) {
      newDropIndex--;
    }

    if (newDropIndex !== dragState.dropTargetIndex) {
      setDragState(prev => ({ ...prev, dropTargetIndex: newDropIndex }));
    }
  }, [dragState.draggedIndex, dragState.isDragging, dragState.dropTargetIndex, containerSelector, itemSelector, createGhost]);

  const handleMouseUp = useCallback(() => {
    const { isDragging, draggedIndex, dropTargetIndex } = dragState;

    if (isDragging && draggedIndex !== null && dropTargetIndex !== null && draggedIndex !== dropTargetIndex) {
      onReorder(draggedIndex, dropTargetIndex);
    }

    removeGhost();
    document.body.style.cursor = '';
    document.body.classList.remove('dragging-active');
    draggedElementRef.current = null;

    if (isDragging) {
      justFinishedDrag.current = true;
      setTimeout(() => {
        justFinishedDrag.current = false;
      }, 100);
    }

    setDragState({
      isDragging: false,
      draggedIndex: null,
      dropTargetIndex: null,
    });
  }, [dragState, onReorder, removeGhost]);

  // Add global mouse listeners
  useEffect(() => {
    if (dragState.draggedIndex !== null) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);

      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [dragState.draggedIndex, handleMouseMove, handleMouseUp]);

  return {
    dragState,
    handleMouseDown,
    justFinishedDrag,
  };
}
