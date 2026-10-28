import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, test } from 'vitest'

import { RegistrationSettings } from './registration-settings'

test('WG remains registered when MASQUE enrollment is pending', () => {
  const html = renderToStaticMarkup(
    createElement(RegistrationSettings, {
      wireGuardStatus: { status: 'registered', hasIpv6: true },
      masqueStatus: { status: 'pending' },
      onResetWireGuard: async () => true,
      onResetMasque: async () => true,
    }),
  )
  const wg = html
    .split('aria-label="Регистрация WireGuard / AWG"')[1]
    .split('</section>')[0]
  const masque = html
    .split('aria-label="Регистрация MASQUE"')[1]
    .split('</section>')[0]
  expect(wg).toContain('Устройство зарегистрировано')
  expect(wg).not.toContain('Регистрация не завершена')
  expect(masque).toContain('Регистрация не завершена')
  expect(html).toContain('Сбросить регистрацию WG / AWG')
  expect(html).toContain('Сбросить регистрацию MASQUE')
})

test('active connection disables both family resets and MASQUE read errors stay separate', () => {
  const html = renderToStaticMarkup(
    createElement(RegistrationSettings, {
      wireGuardStatus: { status: 'registered', hasIpv6: false },
      masqueError: 'Не удалось прочитать регистрацию MASQUE.',
      locked: true,
      onResetWireGuard: async () => true,
      onResetMasque: async () => true,
    }),
  )
  expect(html).toContain('Не удалось прочитать регистрацию MASQUE.')
  expect(html).toContain('Устройство зарегистрировано')
  expect(html.match(/<button[^>]*disabled=""/g)).toHaveLength(2)
  expect(html).toContain('Отключитесь перед сбросом регистрации')
})
