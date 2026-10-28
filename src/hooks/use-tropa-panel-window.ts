import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi'
import { currentMonitor } from '@tauri-apps/api/window'
import { useCallback, useRef } from 'react'

import { useWindowControls } from '@/hooks/use-window'

export function useTropaPanelWindow(onError: (message: string) => void) {
  const { currentWindow } = useWindowControls()
  const compactRef = useRef<{
    size: PhysicalSize
    position: PhysicalPosition
  } | null>(null)
  const queueRef = useRef(Promise.resolve())
  return useCallback(
    (open: boolean) => {
      queueRef.current = queueRef.current
        .then(async () => {
          if (
            (await currentWindow.isMaximized()) ||
            (await currentWindow.isFullscreen())
          )
            return
          const monitor = await currentMonitor()
          if (!monitor) return
          const inner = await currentWindow.innerSize()
          const outer = await currentWindow.outerSize()
          const position = await currentWindow.outerPosition()
          if (open && !compactRef.current)
            compactRef.current = { size: inner, position }
          if (!open && !compactRef.current) return
          const area = monitor.workArea
          const borderWidth = outer.width - inner.width
          const borderHeight = outer.height - inner.height
          const target = open
            ? new PhysicalSize(
                Math.max(inner.width, 1360 * monitor.scaleFactor),
                inner.height,
              )
            : compactRef.current!.size
          const width = Math.min(target.width, area.size.width - borderWidth)
          const height = Math.min(
            target.height,
            area.size.height - borderHeight,
          )
          const origin = open ? position : compactRef.current!.position
          const x = Math.max(
            area.position.x,
            Math.min(
              origin.x,
              area.position.x + area.size.width - width - borderWidth,
            ),
          )
          const y = Math.max(
            area.position.y,
            Math.min(
              origin.y,
              area.position.y + area.size.height - height - borderHeight,
            ),
          )
          await currentWindow.setSize(new PhysicalSize(width, height))
          await currentWindow.setPosition(new PhysicalPosition(x, y))
          if (!open) compactRef.current = null
        })
        .catch(() =>
          onError(
            'Не удалось изменить размер окна. Таблица доступна с прокруткой; окно можно растянуть вручную.',
          ),
        )
    },
    [currentWindow, onError],
  )
}
