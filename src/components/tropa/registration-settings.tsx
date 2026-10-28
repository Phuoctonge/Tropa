import { useState } from 'react'

import type { MasqueAccountStatus, WarpAccountStatus } from '@/types/tropa'

interface RegistrationSettingsProps {
  wireGuardStatus?: WarpAccountStatus
  wireGuardError?: string
  masqueStatus?: MasqueAccountStatus
  masqueError?: string
  locked?: boolean
  busy?: boolean
  onResetWireGuard: () => Promise<boolean>
  onResetMasque?: () => Promise<boolean>
}

export function RegistrationSettings({
  wireGuardStatus,
  wireGuardError,
  masqueStatus,
  masqueError,
  locked = false,
  busy = false,
  onResetWireGuard,
  onResetMasque,
}: RegistrationSettingsProps) {
  const [confirmation, setConfirmation] = useState<
    'wireguard' | 'masque' | null
  >(null)
  const families = [
    {
      id: 'wireguard' as const,
      name: 'WireGuard / AWG',
      actionName: 'WG / AWG',
      status: wireGuardStatus,
      error: wireGuardError,
      reset: onResetWireGuard,
    },
    {
      id: 'masque' as const,
      name: 'MASQUE',
      actionName: 'MASQUE',
      status: masqueStatus,
      error: masqueError,
      reset: onResetMasque,
    },
  ]
  return (
    <>
      <p className="tropa-intro">
        Tropa сохраняет регистрацию устройства для следующих подключений.
        WireGuard и AWG используют одну регистрацию, MASQUE HTTP/3 и HTTP/2 —
        другую.
      </p>
      {families.map((family) => (
        <section
          key={family.id}
          aria-label={`Регистрация ${family.name}`}
          className="tropa-card tropa-explanation"
        >
          <h2 className="tropa-section-label">{family.name}</h2>
          <strong aria-live="polite">
            {family.error
              ? 'Ошибка чтения регистрации'
              : family.status?.status === 'registered'
                ? 'Устройство зарегистрировано'
                : family.status?.status === 'pending'
                  ? 'Регистрация не завершена'
                  : family.status
                    ? 'Устройство ещё не зарегистрировано'
                    : 'Чтение регистрации…'}
          </strong>
          <p>
            {family.error
              ? 'Сохранённые данные не изменены.'
              : family.status?.status === 'registered'
                ? 'Данные сохранены для следующих подключений.'
                : family.status?.status === 'pending'
                  ? 'Следующая попытка продолжит регистрацию с сохранёнными ключами.'
                  : family.id === 'masque'
                    ? 'Регистрация создаётся при первом подключении через MASQUE.'
                    : 'Регистрация создаётся автоматически при первом подключении.'}
          </p>
          {family.error && (
            <div className="tropa-error" role="alert">
              {family.error}
            </div>
          )}
          {confirmation === family.id ? (
            <div
              className="tropa-account-confirm"
              role="group"
              aria-label={`Подтверждение сброса ${family.actionName}`}
            >
              <strong>Сбросить регистрацию {family.actionName}?</strong>
              <p>
                Прежние данные будут заменены после успешного сохранения новой
                регистрации. Регистрация другого семейства сохранится.
              </p>
              <button
                className="tropa-primary"
                disabled={busy || locked || !family.reset}
                onClick={() => {
                  void family.reset?.().then((completed) => {
                    if (completed) setConfirmation(null)
                  })
                }}
              >
                Создать новую регистрацию
              </button>
              <button
                className="tropa-text-button"
                disabled={busy}
                onClick={() => setConfirmation(null)}
              >
                Отмена
              </button>
            </div>
          ) : family.reset &&
            (family.error ||
              family.status?.status === 'registered' ||
              family.status?.status === 'pending') ? (
            <button
              className="tropa-primary"
              disabled={busy || locked}
              onClick={() => setConfirmation(family.id)}
            >
              Сбросить регистрацию {family.actionName}
            </button>
          ) : null}
        </section>
      ))}
      {locked && (
        <p className="tropa-note">Отключитесь перед сбросом регистрации.</p>
      )}
      {busy && (
        <p className="tropa-note" role="status">
          Обработка…
        </p>
      )}
      <p className="tropa-note">
        Данные хранятся локально без шифрования в data/secrets. Копирование
        папки переносит регистрации.
      </p>
    </>
  )
}
