import { type ComponentType, type ReactNode } from 'react'

import { navigationItems } from './_navigation-meta'

export type NavigationItem = {
  label: (typeof navigationItems)[keyof typeof navigationItems]['label']
  path: string
  icon: ReactNode[]
  Component: ComponentType
}

export const navItems: NavigationItem[] = []
