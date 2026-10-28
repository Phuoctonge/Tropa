import { useState } from 'react'

export function DiagnosticsSettings({
  onCopy,
}: {
  onCopy?: () => Promise<boolean>
}) {
  const [copying, setCopying] = useState(false)
  const [copied, setCopied] = useState(false)
  const copy = async () => {
    if (!onCopy || copying) return
    setCopying(true)
    setCopied(false)
    try {
      setCopied(await onCopy())
    } finally {
      setCopying(false)
    }
  }
  return (
    <div className="tropa-card tropa-explanation">
      <strong>Диагностика подключения</strong>
      <p>
        Состояние, коды ошибок, счётчики поиска и хэши файлов. Без ключей,
        логов и путей приложений. Отчёт никуда не отправляется.
      </p>
      <button
        className="tropa-text-button"
        disabled={!onCopy || copying}
        onClick={() => void copy()}
      >
        {copying ? 'Собираем отчёт…' : 'Скопировать диагностику'}
      </button>
      {copied && <p role="status">Отчёт скопирован в буфер обмена.</p>}
    </div>
  )
}
